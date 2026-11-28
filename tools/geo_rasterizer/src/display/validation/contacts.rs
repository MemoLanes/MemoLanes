//! Compare the intersections and junctions of the source and displayed arc graphs.

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use geo::{
    kernels::RobustKernel,
    line_intersection::{line_intersection, LineIntersection},
    Kernel, Orientation,
};
use geo_types::Line;
use rayon::prelude::*;

use crate::display::{
    at_seam,
    spatial::{Bounds, SpatialIndex},
    topology::{ArcState, Graph, NodeId},
    Point,
};

#[derive(Clone, Copy)]
struct Segment {
    arc: usize,
    offset: usize,
    nodes: [NodeId; 2],
    line: Line<f64>,
}

impl Segment {
    fn bounds(self) -> Bounds {
        Bounds::new(self.line.start, self.line.end)
    }

    fn contact(self, other: Self) -> Option<Contact> {
        let (kind, at) = match line_intersection(self.line, other.line)? {
            LineIntersection::Collinear { intersection } => (Kind::Overlap, intersection.start),
            LineIntersection::SinglePoint {
                intersection,
                is_proper,
            } => {
                if !is_proper && self.nodes.iter().any(|node| other.nodes.contains(node)) {
                    return None;
                }
                let kind = if is_proper {
                    Kind::Crossing(RobustKernel::orient2d(
                        self.line.start,
                        self.line.end,
                        other.line.start,
                    ))
                } else {
                    Kind::Touch
                };
                (kind, intersection)
            }
        };
        Some(Contact { kind, at })
    }

    fn along(self, at: Point) -> f64 {
        let d = self.line.end - self.line.start;
        if d.x.abs() > d.y.abs() {
            if d.x > 0.0 {
                at.x
            } else {
                -at.x
            }
        } else if d.y > 0.0 {
            at.y
        } else {
            -at.y
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Touch,
    Crossing(Orientation),
    Overlap,
}

#[derive(Clone, Copy)]
struct Contact {
    kind: Kind,
    at: Point,
}

struct Pair {
    segments: [usize; 2],
    contact: Contact,
}

fn contacts(segments: &[Segment]) -> Vec<Pair> {
    let index = SpatialIndex::new(segments.iter().map(|s| s.bounds()));
    segments
        .par_iter()
        .enumerate()
        .fold(Vec::new, |mut hits, (i, &a)| {
            index.query(a.bounds(), |j| {
                if i < j {
                    if let Some(contact) = a.contact(segments[j]) {
                        hits.push(Pair {
                            segments: [i, j],
                            contact,
                        });
                    }
                }
            });
            hits
        })
        .reduce(Vec::new, |mut a, b| {
            a.extend(b);
            a
        })
}

struct SourceContact {
    // (arc index, source edge offset) for each side.
    edges: [(usize, usize); 2],
    contact: Contact,
}

pub(super) struct SourceContacts {
    pub(super) protected_edges: HashMap<usize, Vec<usize>>,
    hits: Vec<SourceContact>,
    // Source crossings in traversal order on arcs with multiple contacts.
    order: Vec<Vec<(usize, usize, bool)>>,
    junctions: Vec<(NodeId, Vec<ArcEnd>)>,
}

#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
struct ArcEnd {
    arc: usize,
    start: bool,
}

fn circular_order(ends: &mut [ArcEnd], at: Point, ray: impl Fn(ArcEnd) -> Point) {
    let first_half = |p: Point| p.x < at.x || (p.x == at.x && p.y >= at.y);
    ends.sort_unstable_by(|&a, &b| {
        let (u, v) = (ray(a), ray(b));
        first_half(v)
            .cmp(&first_half(u))
            .then_with(|| match RobustKernel::orient2d(at, u, v) {
                Orientation::CounterClockwise => Ordering::Less,
                Orientation::Clockwise => Ordering::Greater,
                Orientation::Collinear => a.cmp(&b),
            })
    });
    // The meridian fixes the start of each half-plane's fan. Two rays on one
    // side can otherwise trade places relative to those on the opposite copy.
    if at_seam(at.x) {
        return;
    }
    let first = ends
        .iter()
        .enumerate()
        .min_by_key(|(_, end)| **end)
        .unwrap()
        .0;
    ends.rotate_left(first);
}

// Pairwise intersection coordinates are computed in f64. Three segments
// meeting at one rational point can produce three different rounded points.
// Preserve all three edges in a triangle of contacts; this also preserves a
// genuine tiny triangular face without having to distinguish the two cases.
fn potential_multiway_contacts(pairs: &[Pair]) -> HashSet<usize> {
    let mut neighbors = HashMap::<usize, HashSet<usize>>::new();
    for &[a, b] in pairs.iter().map(|pair| &pair.segments) {
        neighbors.entry(a).or_default().insert(b);
    }
    let mut fixed = HashSet::new();
    for (&a, adjacent) in &neighbors {
        for &b in adjacent {
            if let Some(others) = neighbors.get(&b) {
                for &c in adjacent.intersection(others) {
                    fixed.extend([a, b, c]);
                }
            }
        }
    }
    fixed
}

impl SourceContacts {
    pub(super) fn new(graph: &Graph) -> Self {
        let mut segments = Vec::new();
        let mut ends = HashMap::<NodeId, Vec<ArcEnd>>::new();
        for (arc, path) in graph.arcs.iter().enumerate() {
            for (offset, pair) in path.nodes.windows(2).enumerate() {
                segments.push(Segment {
                    arc,
                    offset,
                    nodes: [pair[0], pair[1]],
                    line: Line::new(graph.points[pair[0]], graph.points[pair[1]]),
                });
            }
            for (node, start) in [(path.nodes[0], true), (*path.nodes.last().unwrap(), false)] {
                ends.entry(node).or_default().push(ArcEnd { arc, start });
            }
        }
        let junctions = ends
            .into_iter()
            .filter_map(|(node, mut ends)| {
                if ends.len() < 2 || (ends.len() == 2 && !at_seam(graph.points[node].x)) {
                    return None;
                }
                circular_order(&mut ends, graph.points[node], |end| {
                    let path = &graph.arcs[end.arc].nodes;
                    graph.points[path[if end.start { 1 } else { path.len() - 2 }]]
                });
                Some((node, ends))
            })
            .collect();

        let pairs = contacts(&segments);
        let mut protected_edges = HashMap::<usize, Vec<usize>>::new();
        for id in potential_multiway_contacts(&pairs) {
            let s = segments[id];
            protected_edges.entry(s.arc).or_default().push(s.offset);
        }
        let mut by_arc = HashMap::<usize, Vec<_>>::new();
        let hits = pairs
            .into_iter()
            .enumerate()
            .map(|(id, pair)| {
                let edges = pair.segments.map(|i| (segments[i].arc, segments[i].offset));
                for (side, i) in pair.segments.into_iter().enumerate() {
                    let s = segments[i];
                    by_arc.entry(s.arc).or_default().push((
                        s.offset,
                        s.along(pair.contact.at),
                        id,
                        side,
                    ));
                }
                SourceContact {
                    edges,
                    contact: pair.contact,
                }
            })
            .collect();
        let order = by_arc
            .into_values()
            .filter(|hits| hits.len() > 1)
            .map(|mut hits| {
                hits.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));
                hits.iter()
                    .enumerate()
                    .map(|(i, &(edge, position, id, side))| {
                        let tied = i > 0 && (edge, position) == (hits[i - 1].0, hits[i - 1].1);
                        (id, side, tied)
                    })
                    .collect()
            })
            .collect();
        Self {
            protected_edges,
            hits,
            order,
            junctions,
        }
    }

    pub(super) fn conflicts(&self, graph: &Graph, states: &[ArcState]) -> Vec<Conflict> {
        let mut conflicts = seam_conflicts(graph, states);
        let mut segments = Vec::new();
        let mut starts = Vec::with_capacity(graph.arcs.len());
        for (arc, (path, state)) in graph.arcs.iter().zip(states).enumerate() {
            starts.push(segments.len());
            for (span, pair) in path.keep.windows(2).zip(state.points.windows(2)) {
                let line = Line::new(pair[0].degrees(), pair[1].degrees());
                if line.start == line.end {
                    conflicts.push(Conflict {
                        arcs: [arc; 2],
                        reason: "collapsed arc segment",
                        at: line.start,
                    });
                }
                segments.push(Segment {
                    arc,
                    nodes: [path.nodes[span[0]], path.nodes[span[1]]],
                    offset: span[0],
                    line,
                });
            }
        }
        if !conflicts.is_empty() {
            return conflicts;
        }

        let mut expected = HashMap::new();
        let mut seen = vec![None; self.hits.len()];
        for (id, hit) in self.hits.iter().enumerate() {
            let pair = hit.edges.map(|(arc, offset)| {
                starts[arc] + graph.arcs[arc].keep.partition_point(|&k| k <= offset) - 1
            });
            expected.insert(pair, id);
        }
        let pairs = contacts(&segments);
        // Simplifying can also turn separate source crossings into a single
        // new multiway junction. Rounded intersection coordinates alone
        // cannot certify that its three incident edges still form a face.
        for id in potential_multiway_contacts(&pairs) {
            let segment = segments[id];
            let path = &graph.arcs[segment.arc];
            let [a, b] = segment.nodes;
            if path.nodes.get(segment.offset + 1) != Some(&b)
                || segment.line != Line::new(graph.points[a], graph.points[b])
            {
                conflicts.push(Conflict {
                    arcs: [segment.arc; 2],
                    reason: "changed edges at a potential multiway crossing",
                    at: segment.line.start,
                });
            }
        }
        for pair in pairs {
            let Some(&id) = expected.get(&pair.segments) else {
                conflicts.push(Conflict {
                    arcs: pair.segments.map(|i| segments[i].arc),
                    reason: "new boundary contact",
                    at: pair.contact.at,
                });
                continue;
            };
            if pair.contact.kind == self.hits[id].contact.kind {
                seen[id] = Some((pair.segments, pair.contact.at));
            }
        }
        for (id, hit) in self.hits.iter().enumerate() {
            if seen[id].is_none() {
                conflicts.push(Conflict {
                    arcs: hit.edges.map(|e| e.0),
                    reason: "lost or changed source boundary contact",
                    at: hit.contact.at,
                });
            }
        }
        for order in &self.order {
            for window in order.windows(2) {
                let position = |(id, side, _): (usize, usize, bool)| {
                    let (pair, at) = seen[id]?;
                    let i = pair[side];
                    Some((i, segments[i].along(at)))
                };
                if let (Some(a), Some(b)) = (position(window[0]), position(window[1])) {
                    let order_preserved = if window[1].2 { a == b } else { a < b };
                    if !order_preserved {
                        for &(id, _, _) in window {
                            let hit = &self.hits[id];
                            conflicts.push(Conflict {
                                arcs: hit.edges.map(|e| e.0),
                                reason: "source crossing order changed",
                                at: hit.contact.at,
                            });
                        }
                    }
                }
            }
        }
        for (node, original) in &self.junctions {
            let segment = |end: ArcEnd| {
                let offset = if end.start {
                    0
                } else {
                    states[end.arc].points.len() - 2
                };
                segments[starts[end.arc] + offset].line
            };
            let first = original[0];
            let at = if first.start {
                segment(first).start
            } else {
                segment(first).end
            };
            let mut order = original.clone();
            circular_order(&mut order, at, |end| {
                if end.start {
                    segment(end).end
                } else {
                    segment(end).start
                }
            });
            if &order != original {
                for end in original {
                    conflicts.push(Conflict {
                        arcs: [end.arc; 2],
                        reason: "source junction order changed",
                        at: graph.points[*node],
                    });
                }
            }
        }
        conflicts.sort_unstable_by(|a, b| {
            a.arcs
                .cmp(&b.arcs)
                .then(a.reason.cmp(b.reason))
                .then(a.at.x.total_cmp(&b.at.x))
                .then(a.at.y.total_cmp(&b.at.y))
        });
        conflicts.dedup_by(|a, b| a.arcs == b.arcs && a.reason == b.reason);
        conflicts
    }
}

pub(super) struct Conflict {
    pub(super) arcs: [usize; 2],
    pub(super) reason: &'static str,
    pub(super) at: Point,
}

// Cut polygons lie within [-180, 180], so contacts between the two copies can
// occur only on the meridian. The graph nodes every seam edge using the
// latitudes from both copies and splits arcs at each seam node. Preserving
// which points and intervals lie on the seam, and their joint latitude order,
// therefore preserves wrapped contacts without translating or duplicating edges.
fn seam_conflicts(graph: &Graph, states: &[ArcState]) -> Vec<Conflict> {
    let mut conflicts = Vec::new();
    let mut seam_nodes = Vec::new();
    for (arc, (path, state)) in graph.arcs.iter().zip(states).enumerate() {
        for (&offset, &vertex) in path.keep.iter().zip(&state.points) {
            let point = vertex.degrees();
            if !at_seam(point.x) {
                continue;
            }
            let source = graph.points[path.nodes[offset]];
            if point.x != source.x {
                conflicts.push(Conflict {
                    arcs: [arc; 2],
                    reason: "new dateline contact",
                    at: point,
                });
            } else {
                seam_nodes.push((source.y, point, arc));
            }
        }
        // Between two consecutive seam junctions the arc can bow away from
        // the meridian. A shortcut between its ends must not cover that interval.
        if path.keep.len() == 2 && path.nodes.len() > 2 {
            let start = state.points[0].degrees();
            let end = state.points[1].degrees();
            if at_seam(start.x) && start.x == end.x {
                conflicts.push(Conflict {
                    arcs: [arc; 2],
                    reason: "new dateline edge",
                    at: start,
                });
            }
        }
    }
    seam_nodes.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    for &[(s, a, arc), (t, b, other)] in seam_nodes.array_windows() {
        if (s < t && a.y >= b.y) || (s == t && a.y != b.y) {
            conflicts.push(Conflict {
                arcs: [arc, other],
                reason: "dateline contact order changed",
                at: a,
            });
        }
    }
    conflicts
}

#[cfg(test)]
mod tests {
    use crate::display::{build_display_records, DisplaySource};
    use geo_data_format::{GeoEntityId, Worldview};
    use geo_types::{LineString, MultiPolygon, Polygon};

    #[test]
    fn preserves_three_edges_meeting_at_one_nonrepresentable_interior_point() {
        // The first edge of each triangle passes through (1/96, 1/48).
        // Coarse rounding splits the junction into three ordinary crossings
        // with the same apparent order under floating-point intersection.
        let triangles = [
            [(-3., 24.), (1., -4.), (-11., 17.)],
            [(-3., 14.), (1., -2.), (9., 11.)],
            [(-1., 2.), (4., -3.), (-2., -11.)],
        ]
        .map(|triangle| triangle.map(|(x, y)| (x / 32., y / 32.)));
        let sources: Vec<_> = triangles
            .iter()
            .enumerate()
            .map(|(id, &[a, b, c])| DisplaySource {
                entity_id: GeoEntityId(id as u32),
                polygons: MultiPolygon(vec![Polygon::new(
                    LineString::from(vec![a, b, c, a]),
                    vec![],
                )]),
            })
            .collect();
        let records = build_display_records(Worldview::Iso, &sources).unwrap();
        assert_eq!(records.len(), triangles.len());
        for (record, [a, b, _]) in records.into_iter().zip(triangles) {
            let [a, b] = [a, b].map(|(x, y)| [x, y]);
            let polygons = record.polygons.into_degrees();
            assert!(
                polygons[0][0]
                    .windows(2)
                    .any(|edge| edge == [a, b] || edge == [b, a]),
                "source edge missing from ID {}",
                record.entity_id.0
            );
        }
    }
}
