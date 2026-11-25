//! Reject crossings and shared segments not supported by the source paths.

use std::{collections::HashMap, ops::Range};

use geo::{
    kernels::RobustKernel,
    line_intersection::{line_intersection, LineIntersection},
    Kernel, Orientation,
};
use geo_types::Line;
use rayon::prelude::*;

use crate::display::{
    spatial::{Bounds, SpatialIndex},
    topology::{ArcState, Graph, GridPoint, NodeId, RingId},
    Point,
};

struct Chord {
    arc: usize,
    lo: usize,
    hi: usize,
    line: Line<f64>,
}

// A geometric contact between two different output chords must be supported
// by the original subpaths. In particular, distinct arcs may not be merged by
// snapping, and crossings may not be introduced where the source was disjoint.
fn has_source_contact(graph: &Graph, a: &Chord, b: &Chord, hit: LineIntersection<f64>) -> bool {
    let path = |c: &Chord| &graph.arcs[c.arc].nodes[c.lo..=c.hi];
    let line = |e: &[NodeId]| Line::new(graph.points[e[0]], graph.points[e[1]]);
    let compatible = |old: LineIntersection<f64>| match hit {
        LineIntersection::Collinear { .. } => matches!(old, LineIntersection::Collinear { .. }),
        LineIntersection::SinglePoint { .. } => true,
    };
    path(a).windows(2).any(|ae| {
        path(b)
            .windows(2)
            .any(|be| line_intersection(line(ae), line(be)).is_some_and(compatible))
    })
}

// Chord indices are consecutive inside each arc. At arc ends, follow the
// actual uses in the rings: a junction can have more than one continuation.
// The ranges omit collapsed chords, so these are also the neighbors after
// deduplicating coincident snapped vertices.
struct ChordLinks {
    by_arc: Vec<Range<usize>>,
    junctions: HashMap<(usize, bool), Vec<Continuation>>,
}

struct Continuation {
    chord: usize,
    rings: Vec<RingId>,
}

#[derive(Clone, Copy)]
struct ChordPath<'a> {
    neighbor: Option<usize>,
    // At a junction only these rings take this continuation. None means
    // every user of the base chord takes the same path through the contact.
    rings: Option<&'a [RingId]>,
}

impl ChordLinks {
    fn new(graph: &Graph, enabled: &[bool], by_arc: Vec<Range<usize>>) -> Self {
        let mut junctions: HashMap<_, Vec<Continuation>> = HashMap::new();
        for (ring_id, ring) in graph
            .rings
            .iter()
            .enumerate()
            .filter(|(_, ring)| enabled[ring.owner])
        {
            let mut walk = ring
                .arcs
                .iter()
                .filter_map(|&(arc, forward)| {
                    let range = &by_arc[arc];
                    let (first, last) = (range.clone().next()?, range.clone().next_back()?);
                    Some(if forward {
                        (first, last, true)
                    } else {
                        (last, first, false)
                    })
                })
                .peekable();
            let Some(&first) = walk.peek() else { continue };
            let mut prior = first;
            for next in walk.skip(1).chain([first]) {
                let (last, first) = ((prior.1, !prior.2), (next.0, next.2));
                for (end, other) in [(last, first.0), (first, last.0)] {
                    let paths = junctions.entry(end).or_default();
                    if let Some(path) = paths.iter_mut().find(|path| path.chord == other) {
                        if path.rings.last() != Some(&ring_id) {
                            path.rings.push(ring_id);
                        }
                    } else {
                        paths.push(Continuation {
                            chord: other,
                            rings: vec![ring_id],
                        });
                    }
                }
                prior = next;
            }
        }
        Self { by_arc, junctions }
    }

    fn paths_at(
        &self,
        chords: &[Chord],
        id: usize,
        at: Point,
    ) -> impl Iterator<Item = ChordPath<'_>> + '_ {
        let c = &chords[id];
        let range = &self.by_arc[c.arc];
        let (single, junction) = if c.line.start == at {
            if id > range.start {
                (Some(Some(id - 1)), None)
            } else {
                (None, self.junctions.get(&(id, true)))
            }
        } else if c.line.end == at {
            if id + 1 < range.end {
                (Some(Some(id + 1)), None)
            } else {
                (None, self.junctions.get(&(id, false)))
            }
        } else {
            (Some(None), None)
        };
        single
            .into_iter()
            .map(|neighbor| ChordPath {
                neighbor,
                rings: None,
            })
            .chain(junction.into_iter().flatten().map(|path| ChordPath {
                neighbor: Some(path.chord),
                rings: Some(&path.rings),
            }))
    }
}

fn rays_cross(at: Point, a: [Point; 2], b: [Point; 2]) -> bool {
    use Orientation::{Clockwise, CounterClockwise};
    // Strictly alternate around the intersection. A tangent or a pair of
    // collinear rays is not a crossing (positive-length contacts are checked
    // separately). A straight ray pair can still straddle a bent pair.
    let orientation = |u, v| RobustKernel::orient2d(at, u, v);
    let between = |[u, v]: [Point; 2], p| {
        let turns = [orientation(u, p), orientation(p, v)];
        if orientation(u, v) == Clockwise {
            turns.contains(&CounterClockwise)
        } else {
            turns == [CounterClockwise, CounterClockwise]
        }
    };
    let same_ray = |u: Point, v: Point| {
        let (u_ray, v_ray) = (u - at, v - at);
        orientation(u, v) == Orientation::Collinear && u_ray.x * v_ray.x + u_ray.y * v_ray.y > 0.0
    };
    // Opposite rays are distinct; coincident rays are not strict alternation.
    if a.into_iter().any(|u| b.into_iter().any(|v| same_ray(u, v))) {
        return false;
    }
    between(a, b[0]) != between(a, b[1])
}

fn output_rays(chords: &[Chord], id: usize, neighbor: Option<usize>, at: Point) -> [Point; 2] {
    let ends = |id: usize| [chords[id].line.start, chords[id].line.end];
    let away = |id: usize| ends(id).into_iter().find(|&p| p != at).unwrap();
    neighbor.map_or_else(|| ends(id), |n| [away(id), away(n)])
}

fn source_paths(
    graph: &Graph,
    chords: &[Chord],
    id: usize,
    neighbor: Option<usize>,
) -> Vec<Vec<Point>> {
    let points = |id: usize| {
        let c = &chords[id];
        graph.arcs[c.arc].nodes[c.lo..=c.hi]
            .iter()
            .map(|&n| graph.points[n])
            .collect::<Vec<_>>()
    };
    let mut path = points(id);
    let Some(neighbor) = neighbor else {
        return vec![path];
    };
    let mut other = points(neighbor);
    if path.first() == other.first() || path.first() == other.last() {
        path.reverse();
    }
    if path.last() == other.last() {
        other.reverse();
    }
    if path.last() == other.first() {
        path.extend(other.into_iter().skip(1));
        vec![path]
    } else {
        // The snapped join has collapsed some intervening source edges. Do
        // not invent a source chord to bridge the gap for this comparison.
        vec![path, other]
    }
}

fn source_crosses(a: &[Point], b: &[Point]) -> bool {
    let line = |edge: &[Point]| Line::new(edge[0], edge[1]);
    let rays = |path: &[Point], edge: usize, at: Point| {
        if at == path[edge] {
            Some([*path.get(edge.checked_sub(1)?)?, path[edge + 1]])
        } else if at == path[edge + 1] {
            Some([path[edge], *path.get(edge + 2)?])
        } else {
            Some([path[edge], path[edge + 1]])
        }
    };
    a.windows(2).enumerate().any(|(i, a_edge)| {
        b.windows(2).enumerate().any(|(j, b_edge)| {
            match line_intersection(line(a_edge), line(b_edge)) {
                Some(LineIntersection::SinglePoint {
                    is_proper: true, ..
                }) => true,
                Some(LineIntersection::SinglePoint {
                    intersection,
                    is_proper: false,
                }) => rays(a, i, intersection)
                    .zip(rays(b, j, intersection))
                    .is_some_and(|(a, b)| rays_cross(intersection, a, b)),
                _ => false,
            }
        })
    })
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ConflictSide {
    pub(super) arc: usize,
    pub(super) continuation: Option<usize>,
    rings: Option<Vec<RingId>>,
}

impl ConflictSide {
    pub(super) fn owners(&self, graph: &Graph, enabled: &[bool]) -> Vec<usize> {
        let mut ids: Vec<_> = if let Some(rings) = &self.rings {
            rings
                .iter()
                .map(|&r| graph.rings[r].owner)
                .filter(|&i| enabled[i])
                .collect()
        } else {
            graph.arcs[self.arc]
                .owners
                .iter()
                .copied()
                .filter(|&i| enabled[i])
                .collect()
        };
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Conflict {
    pub(super) sides: [ConflictSide; 2],
    pub(super) shared_segment: bool,
    pub(super) at: GridPoint,
}

pub(super) fn new_contacts(graph: &Graph, states: &[ArcState], enabled: &[bool]) -> Vec<Conflict> {
    let mut chords = vec![];
    let mut by_arc = Vec::with_capacity(graph.arcs.len());
    for (id, (arc, state)) in graph.arcs.iter().zip(states).enumerate() {
        let start = chords.len();
        if !arc.owners.iter().any(|&i| enabled[i]) {
            by_arc.push(start..start);
            continue;
        }
        for (span, pair) in arc.keep.windows(2).zip(state.points.windows(2)) {
            if pair[0] == pair[1] {
                continue;
            }
            // Do intersection predicates in integer-valued f64s, avoiding
            // rounding a second time when testing whether snapped lines meet.
            let point = |p: GridPoint| (f64::from(p[0]), f64::from(p[1]));
            chords.push(Chord {
                arc: id,
                lo: span[0],
                hi: span[1],
                line: Line::new(point(pair[0]), point(pair[1])),
            });
        }
        by_arc.push(start..chords.len());
    }
    let links = ChordLinks::new(graph, enabled, by_arc);
    let bounds = |chord: &Chord| Bounds::new(chord.line.start, chord.line.end);
    let index = SpatialIndex::new(chords.iter().map(bounds));
    let mut conflicts: Vec<_> = chords
        .par_iter()
        .enumerate()
        .fold(Vec::new, |mut conflicts, (i, a)| {
            index.query(bounds(a), |j| {
                if i >= j {
                    return;
                }
                let b = &chords[j];
                let Some(hit) = line_intersection(a.line, b.line) else {
                    return;
                };
                let mut conflict =
                    |shared_segment, point: Point, [a_path, b_path]: [ChordPath<'_>; 2]| {
                        let sides = [(a, a_path), (b, b_path)].map(|(chord, path)| ConflictSide {
                            arc: chord.arc,
                            continuation: path.neighbor.map(|n| chords[n].arc),
                            rings: path.rings.map(<[RingId]>::to_vec),
                        });
                        conflicts.push(Conflict {
                            sides,
                            shared_segment,
                            at: [point.x.round() as i32, point.y.round() as i32],
                        });
                    };
                let any_ring = ChordPath {
                    neighbor: None,
                    rings: None,
                };
                match hit {
                    LineIntersection::SinglePoint {
                        intersection,
                        is_proper: false,
                    } => {
                        for a_path in links.paths_at(&chords, i, intersection) {
                            for b_path in links.paths_at(&chords, j, intersection) {
                                if !rays_cross(
                                    intersection,
                                    output_rays(&chords, i, a_path.neighbor, intersection),
                                    output_rays(&chords, j, b_path.neighbor, intersection),
                                ) {
                                    continue;
                                }
                                let ap = source_paths(graph, &chords, i, a_path.neighbor);
                                let bp = source_paths(graph, &chords, j, b_path.neighbor);
                                if !ap.iter().any(|a| bp.iter().any(|b| source_crosses(a, b))) {
                                    conflict(false, intersection, [a_path, b_path]);
                                }
                            }
                        }
                    }
                    LineIntersection::SinglePoint { intersection, .. } => {
                        if !has_source_contact(graph, a, b, hit) {
                            conflict(false, intersection, [any_ring; 2]);
                        }
                    }
                    LineIntersection::Collinear { intersection } => {
                        if !has_source_contact(graph, a, b, hit) {
                            conflict(true, intersection.start, [any_ring; 2]);
                        }
                    }
                }
            });
            conflicts
        })
        .reduce(Vec::new, |mut a, b| {
            a.extend(b);
            a
        });
    conflicts.sort_unstable();
    conflicts.dedup_by(|a, b| a.sides == b.sides && a.shared_segment == b.shared_segment);
    conflicts
}
