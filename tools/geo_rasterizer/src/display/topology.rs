//! Node exact source boundaries, then simplify and snap shared arcs jointly.

use std::collections::HashMap;

use geo::{kernels::RobustKernel, Kernel, Orientation};
use geo_types::{LineString, Polygon};

use super::{
    at_seam, parameter, point_key, rings,
    source::Prepared,
    spatial::{Bounds, SpatialIndex},
    Point,
};

const UNIT: f64 = 100_000.0;
const COARSE: f64 = 10.0; // Multiples of 0.0001°, in storage units.
const FINE: f64 = 1.0;

pub(super) type GridPoint = [i32; 2];
pub(super) type RingId = usize;
pub(super) type NodeId = usize;
pub(super) type Shape = Vec<Vec<Vec<Vertex>>>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Vertex {
    Grid(GridPoint),
    Exact(Point),
}

impl Vertex {
    pub(super) fn degrees(self) -> Point {
        match self {
            Self::Grid(p) => degrees(p),
            Self::Exact(p) => p,
        }
    }

    pub(super) fn grid(self) -> Option<GridPoint> {
        match self {
            Self::Grid(p) => Some(p),
            Self::Exact(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Precision {
    Coarse,
    Fine,
    Exact,
}

struct Edge {
    ends: [NodeId; 2],
    rings: Vec<RingId>,
    arc: usize,
}

struct SourceRing {
    owner: usize,
    nodes: Vec<NodeId>, // excludes closure until graph noding
    edges: Vec<usize>,
}

pub(super) struct Ring {
    pub(super) owner: usize,
    pub(super) arcs: Vec<(usize, bool)>, // arc index and traversal direction (forward)
}

pub(super) struct Arc {
    pub(super) nodes: Vec<NodeId>,
    pub(super) owners: Vec<usize>,
    pub(super) keep: Vec<usize>,
    pub(super) precision: Precision,
}

pub(super) struct Graph {
    pub(super) points: Vec<Point>,
    pub(super) rings: Vec<Ring>,
    pub(super) arcs: Vec<Arc>,
    node_precision: Vec<Precision>,
    // Paired -180/+180 nodes with the same latitude must quantize together.
    seam_mates: Vec<Option<NodeId>>,
    // Per source: polygons → rings, exterior first.
    pub(super) shapes: Vec<Vec<Vec<RingId>>>,
}

fn mix(a: Point, b: Point, t: f64) -> Point {
    Point {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
    }
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn canonicalize_arc(nodes: &mut [NodeId], points: &[Point], junctions: &[bool]) {
    let compare = |a: NodeId, b: NodeId| {
        let (a, b) = (points[a], points[b]);
        a.x.total_cmp(&b.x).then_with(|| a.y.total_cmp(&b.y))
    };
    let end = nodes.len() - 1;
    let order = if nodes[0] == nodes[end] {
        // A loop attached to a junction must start there in every incident ring.
        if !junctions[nodes[0]] {
            let start = (0..end)
                .min_by(|&a, &b| compare(nodes[a], nodes[b]))
                .unwrap();
            nodes[..end].rotate_left(start);
            nodes[end] = nodes[0];
        }
        compare(nodes[1], nodes[end - 1])
    } else {
        compare(nodes[0], nodes[end])
    };
    if order.is_gt() {
        nodes.reverse();
    }
}

fn simplify(nodes: &[NodeId], points: &[Point], budget: f64, pinned: &[usize]) -> Vec<usize> {
    let mut keep = vec![false; nodes.len()];
    let mut pending: Vec<_> = pinned.windows(2).map(|pair| (pair[0], pair[1])).collect();
    while let Some((lo, hi)) = pending.pop() {
        keep[lo] = true;
        keep[hi] = true;
        let (a, b) = (points[nodes[lo]], points[nodes[hi]]);
        let mut worst = (budget, lo);
        let mut previous = 0.0;
        for i in lo + 1..hi {
            let p = points[nodes[i]];
            let t = parameter(a, b, p);
            // Monotone projected parameters give a pointwise correspondence
            // in both directions, not just a one-sided vertex-to-line test.
            if !t.is_finite() || t < previous || t > 1.0 {
                worst.1 = i;
                break;
            }
            previous = t;
            let err = distance(p, mix(a, b, t));
            if err > worst.0 {
                worst = (err, i);
            }
        }
        if worst.1 != lo {
            pending.extend([(lo, worst.1), (worst.1, hi)]);
        }
    }
    keep.into_iter()
        .enumerate()
        .filter_map(|(i, yes)| yes.then_some(i))
        .collect()
}

impl Graph {
    pub(super) fn build(sources: &[Prepared]) -> Self {
        let mut points = vec![];
        let mut ids = HashMap::new();
        let mut source_rings = vec![];
        let shapes = sources
            .iter()
            .enumerate()
            .map(|(owner, source)| {
                source
                    .geometry
                    .0
                    .iter()
                    .map(|polygon| {
                        rings(polygon)
                            .map(|ring| {
                                let nodes = ring.0[..ring.0.len() - 1]
                                    .iter()
                                    .map(|&p| {
                                        *ids.entry(point_key(p)).or_insert_with(|| {
                                            points.push(p);
                                            points.len() - 1
                                        })
                                    })
                                    .collect();
                                source_rings.push(SourceRing {
                                    owner,
                                    nodes,
                                    edges: vec![],
                                });
                                source_rings.len() - 1
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect();

        // Add opposite-dateline vertices before noding the artificial cuts.
        // Thus differently segmented cuts also stay aligned when a ring retries.
        let mut seam_mates = vec![None; points.len()];
        for i in 0..points.len() {
            let p = points[i];
            if !at_seam(p.x) {
                continue;
            }
            let other = Point { x: -p.x, y: p.y };
            let mate = *ids.entry(point_key(other)).or_insert_with(|| {
                points.push(other);
                seam_mates.push(None);
                points.len() - 1
            });
            seam_mates[i] = Some(mate);
            seam_mates[mate] = Some(i);
        }
        let spatial = SpatialIndex::new(points.iter().map(|&p| Bounds::new(p, p)));
        let mut edge_ids = HashMap::new();
        let mut edges: Vec<Edge> = vec![];
        let mut adjacent = vec![vec![]; points.len()];
        let mut split = vec![];
        for (ring_id, ring) in source_rings.iter_mut().enumerate() {
            let original = std::mem::take(&mut ring.nodes);
            for (&from, &to) in original.iter().zip(original.iter().cycle().skip(1)) {
                let (a, b) = (points[from], points[to]);
                split.clear();
                split.extend([from, to]);
                spatial.query(Bounds::new(a, b), |id| {
                    if id != from
                        && id != to
                        && RobustKernel::orient2d(a, b, points[id]) == Orientation::Collinear
                    {
                        split.push(id);
                    }
                });
                // On a long segment the projected fraction can round to 1 for
                // a distinct interior vertex immediately before the endpoint.
                let ordinate = |p: Point| if a.x == b.x { p.y } else { p.x };
                let forward = ordinate(a) < ordinate(b);
                split.sort_unstable_by(|&u, &v| {
                    let order = ordinate(points[u]).total_cmp(&ordinate(points[v]));
                    if forward {
                        order
                    } else {
                        order.reverse()
                    }
                });
                for &[a, b] in split.array_windows() {
                    let ends = if a < b { [a, b] } else { [b, a] };
                    let eid = *edge_ids.entry(ends).or_insert_with(|| {
                        let eid = edges.len();
                        edges.push(Edge {
                            ends,
                            rings: vec![],
                            arc: usize::MAX,
                        });
                        adjacent[a].push(eid);
                        adjacent[b].push(eid);
                        eid
                    });
                    if edges[eid].rings.last() != Some(&ring_id) {
                        edges[eid].rings.push(ring_id);
                    }
                    ring.nodes.push(a);
                    ring.edges.push(eid);
                }
            }
        }
        let junctions: Vec<_> = adjacent
            .iter()
            .enumerate()
            .map(|(i, e)| {
                e.len() != 2 || at_seam(points[i].x) || edges[e[0]].rings != edges[e[1]].rings
            })
            .collect();
        let mut arcs = Vec::new();
        for id in 0..edges.len() {
            if edges[id].arc != usize::MAX {
                continue;
            }
            let [a, b] = edges[id].ends;
            let owners = edges[id]
                .rings
                .iter()
                .map(|&r| source_rings[r].owner)
                .collect::<Vec<_>>();
            // Mark this edge before walking, so a pure cycle stops at itself.
            edges[id].arc = arcs.len();
            let mut extend = |mut at: NodeId, mut edge: usize, nodes: &mut Vec<NodeId>| {
                nodes.push(at);
                while !junctions[at] {
                    let next = adjacent[at][usize::from(adjacent[at][0] == edge)];
                    if edges[next].arc != usize::MAX {
                        break;
                    }
                    edges[next].arc = arcs.len();
                    let [u, v] = edges[next].ends;
                    at = if u == at { v } else { u };
                    nodes.push(at);
                    edge = next;
                }
            };
            let mut nodes = vec![];
            extend(a, id, &mut nodes);
            nodes.reverse();
            if nodes.first() != Some(&b) {
                extend(b, id, &mut nodes);
            } else {
                nodes.push(b);
            }
            canonicalize_arc(&mut nodes, &points, &junctions);
            arcs.push(Arc {
                nodes,
                owners,
                keep: Vec::new(),
                precision: Precision::Coarse,
            });
        }
        let mut rings = Vec::with_capacity(source_rings.len());
        for ring in source_rings {
            let start = ring
                .nodes
                .iter()
                .enumerate()
                .position(|(i, &node)| {
                    let arc = &arcs[edges[ring.edges[i]].arc];
                    let next = ring.nodes[(i + 1) % ring.nodes.len()];
                    forward(&arc.nodes, node, next).is_some()
                })
                .unwrap();
            let mut step = 0;
            let mut directed_arcs = Vec::new();
            while step < ring.nodes.len() {
                let i = (start + step) % ring.nodes.len();
                let id = edges[ring.edges[i]].arc;
                let next = ring.nodes[(i + 1) % ring.nodes.len()];
                directed_arcs.push((id, forward(&arcs[id].nodes, ring.nodes[i], next).unwrap()));
                step += arcs[id].nodes.len() - 1;
            }
            debug_assert_eq!(step, ring.nodes.len());
            rings.push(Ring {
                owner: ring.owner,
                arcs: directed_arcs,
            });
        }
        Self {
            node_precision: vec![Precision::Coarse; points.len()],
            points,
            rings,
            arcs,
            seam_mates,
            shapes,
        }
    }

    pub(super) fn simplify(
        &mut self,
        sources: &[Prepared],
        protected_edges: &HashMap<usize, Vec<usize>>,
    ) {
        for (id, arc) in self.arcs.iter_mut().enumerate() {
            let mut pinned = vec![0, arc.nodes.len() - 1];
            if let Some(edges) = protected_edges.get(&id) {
                for &offset in edges {
                    for i in [offset, offset + 1] {
                        pinned.push(i);
                        let node = arc.nodes[i];
                        self.node_precision[node] = Precision::Exact;
                        if let Some(mate) = self.seam_mates[node] {
                            self.node_precision[mate] = Precision::Exact;
                        }
                    }
                }
                pinned.sort_unstable();
                pinned.dedup();
            }
            let budget = arc
                .owners
                .iter()
                .map(|&i| sources[i].budget)
                .fold(f64::INFINITY, f64::min);
            arc.keep = simplify(&arc.nodes, &self.points, budget * 0.8, &pinned);
        }
    }

    pub(super) fn back_off_ring(&mut self, ring: RingId, precision: Precision) -> bool {
        let mut changed = false;
        for (arc, _) in self.rings[ring].arcs.clone() {
            changed |= self.back_off_arc(arc, precision);
        }
        changed
    }

    pub(super) fn back_off_shape(&mut self, owner: usize, precision: Precision) -> bool {
        let arcs: Vec<_> = self.shapes[owner]
            .iter()
            .flatten()
            .flat_map(|&r| self.rings[r].arcs.iter().map(|a| a.0))
            .collect();
        let mut changed = false;
        for arc in arcs {
            changed |= self.back_off_arc(arc, precision);
        }
        changed
    }

    pub(super) fn back_off_arc(&mut self, id: usize, precision: Precision) -> bool {
        let arc = &mut self.arcs[id];
        if arc.precision >= precision {
            return false;
        }
        arc.precision = precision;
        arc.keep = (0..arc.nodes.len()).collect();
        for &node in &arc.nodes {
            self.node_precision[node] = self.node_precision[node].max(precision);
            if let Some(mate) = self.seam_mates[node] {
                self.node_precision[mate] = self.node_precision[mate].max(precision);
            }
        }
        true
    }

    fn vertex(&self, node: NodeId) -> Vertex {
        let grid = match self.node_precision[node] {
            Precision::Coarse => COARSE,
            Precision::Fine => FINE,
            Precision::Exact => return Vertex::Exact(self.points[node]),
        };
        let q = |x: f64| ((x * UNIT / grid).round() * grid) as i32;
        Vertex::Grid([q(self.points[node].x), q(self.points[node].y)])
    }
}

fn forward(arc: &[NodeId], from: NodeId, to: NodeId) -> Option<bool> {
    if arc[..2] == [from, to] {
        Some(true)
    } else if arc[arc.len() - 2..] == [to, from] {
        Some(false)
    } else {
        None
    }
}

pub(super) fn degrees(p: GridPoint) -> Point {
    (f64::from(p[0]) / UNIT, f64::from(p[1]) / UNIT).into()
}

pub(super) fn polygon_from_vertices(rings: &[Vec<Vertex>]) -> Polygon<f64> {
    let ring =
        |points: &Vec<Vertex>| LineString(points.iter().copied().map(Vertex::degrees).collect());
    Polygon::new(ring(&rings[0]), rings[1..].iter().map(ring).collect())
}

pub(super) struct ArcState {
    pub(super) points: Vec<Vertex>,
    pub(super) error: f64,
}

impl ArcState {
    pub(super) fn new(graph: &Graph, arc: &Arc) -> Self {
        let points: Vec<_> = arc
            .keep
            .iter()
            .map(|&i| graph.vertex(arc.nodes[i]))
            .collect();
        let mut error: f64 = 0.0;
        for (range, pair) in arc.keep.windows(2).zip(points.windows(2)) {
            let a = graph.points[arc.nodes[range[0]]];
            let b = graph.points[arc.nodes[range[1]]];
            let (qa, qb) = (pair[0].degrees(), pair[1].degrees());
            error = error.max(distance(a, qa)).max(distance(b, qb));
            for &node in &arc.nodes[range[0] + 1..range[1]] {
                let p = graph.points[node];
                error = error.max(distance(p, mix(qa, qb, parameter(a, b, p))));
            }
        }
        Self { points, error }
    }
}
