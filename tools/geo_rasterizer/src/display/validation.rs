//! Materialize and check each snapshot before refining or omitting outlines.

mod contacts;
mod overlaps;

use std::collections::BTreeMap;

use anyhow::{ensure, Result};
use geo::{winding_order::Winding, Area, BooleanOps, Euclidean, Length};
use geo_data_format::{DisplayRecord, Worldview};
use geo_types::{LineString, MultiPolygon};
use rayon::prelude::*;

use super::{
    rings,
    source::{valid_geometry, Prepared},
    topology::{degrees, polygon_from_grid, ArcState, Graph, Shape},
};
use contacts::{new_contacts, Conflict};
use overlaps::{new_interior_overlaps, InteriorOverlapCache};

fn validated_shape(
    graph: &Graph,
    owner: usize,
    states: &[ArcState],
    source: &Prepared,
) -> Result<Shape> {
    let mut shape = vec![];
    for (ring_ids, reference) in graph.shapes[owner].iter().zip(&source.geometry.0) {
        let mut polygon = vec![];
        for (&r, original) in ring_ids.iter().zip(rings(reference)) {
            let mut ring = vec![];
            for &(a, forward) in &graph.rings[r].arcs {
                let state = &states[a];
                ensure!(
                    state.error <= source.budget,
                    "combined fit error {:.7}° > {:.7}°",
                    state.error,
                    source.budget
                );
                let path = &state.points;
                if forward {
                    ring.extend(path[..path.len() - 1].iter().copied());
                } else {
                    ring.extend(path[1..].iter().rev().copied());
                }
            }
            ring.dedup();
            if ring.last() != ring.first() {
                ring.push(ring[0]);
            }
            ensure!(ring.len() >= 4, "ring collapsed on grid");
            let snapped = LineString(ring.iter().copied().map(degrees).collect());
            ensure!(
                snapped.winding_order().is_some()
                    && snapped.winding_order() == original.winding_order(),
                "ring collapsed or changed winding"
            );
            polygon.push(ring);
        }
        shape.push(polygon);
    }
    let candidate = MultiPolygon(
        shape
            .iter()
            .map(|rings| polygon_from_grid(rings, degrees))
            .collect(),
    );
    valid_geometry(&candidate)?;
    for (before, after) in source.geometry.0.iter().zip(&candidate.0) {
        // Check the actual covered set, not just area totals (which can
        // mask loss on one side with a gain on the other).
        let swept = rings(before).map(|r| Euclidean.length(r)).sum::<f64>() * source.budget;
        let changed = before.xor(after).unsigned_area();
        ensure!(
            changed <= swept + std::f64::consts::PI * source.budget.powi(2),
            "covered area exceeds fit budget"
        );
    }
    Ok(shape)
}

pub(super) fn validated_records(
    worldview: Worldview,
    prepared: &[Prepared],
    source_count: usize,
    prefix: &str,
) -> Vec<DisplayRecord> {
    let mut graph = Graph::build(prepared);
    let mut enabled = vec![true; prepared.len()];
    let mut fallback = BTreeMap::new();
    let mut overlap_cache = InteriorOverlapCache::default();
    let extent = |i: usize| {
        let b = prepared[i].bounds;
        (b.east - b.west).max(b.north - b.south)
    };
    loop {
        let states: Vec<_> = graph
            .arcs
            .par_iter()
            .map(|a| ArcState::new(&graph, a))
            .collect();
        let shapes: Vec<_> = prepared
            .par_iter()
            .enumerate()
            .map(|(i, s)| enabled[i].then(|| validated_shape(&graph, i, &states, s)))
            .collect();
        let mut failures = shapes
            .iter()
            .enumerate()
            .filter_map(|(i, shape)| shape.as_ref()?.as_ref().err().map(|error| (i, error)))
            .peekable();
        let mut retry = false;
        for (i, error) in failures.clone() {
            if graph.back_off_shape(i) {
                fallback.entry(i).or_insert_with(|| format!("{error:#}"));
                retry = true;
            }
        }
        // All failures refer to the same snapshot. A preceding request may
        // already have refined this shape's shared arcs or endpoints.
        if retry {
            continue;
        }
        if failures.peek().is_some() {
            for (i, error) in failures {
                enabled[i] = false;
                eprintln!(
                    "{prefix} id={} unavailable at fine grid: {error:#}",
                    prepared[i].id.0
                );
            }
            continue;
        }
        let conflicts = new_contacts(&graph, &states, &enabled);
        for Conflict { sides, .. } in &conflicts {
            let mut changed = false;
            for side in sides {
                changed |= graph.back_off_arc(side.arc);
                if let Some(arc) = side.continuation {
                    changed |= graph.back_off_arc(arc);
                }
            }
            if changed {
                for i in sides.iter().flat_map(|side| side.owners(&graph, &enabled)) {
                    fallback.entry(i).or_insert_with(|| {
                        "new crossing or shared segment between distinct arcs".into()
                    });
                }
                retry = true;
            }
        }
        if retry {
            continue;
        }
        let overlaps = new_interior_overlaps(prepared, &shapes, &mut overlap_cache);
        for &[left, right] in &overlaps {
            for (i, other) in [(left, right), (right, left)] {
                if graph.back_off_shape(i) {
                    fallback.insert(
                        i,
                        format!(
                            "new component interior overlap (other ID {})",
                            prepared[other].id.0
                        ),
                    );
                    retry = true;
                }
            }
        }
        if retry {
            continue;
        }
        for Conflict {
            sides,
            shared_segment,
            at,
        } in conflicts
        {
            let [left, right] = sides.map(|side| side.owners(&graph, &enabled));
            if left.is_empty() || right.is_empty() {
                continue;
            }
            // Both sides have used all stored precision. Keep the side with
            // larger geographic extent if the conflict can't be resolved.
            let strength =
                |owners: &[usize]| owners.iter().copied().map(extent).fold(0.0, f64::max);
            let (discard, kept) = if strength(&left) > strength(&right) {
                (&right, &left)
            } else {
                (&left, &right)
            };
            let kept: Vec<_> = kept.iter().map(|&i| prepared[i].id.0).collect();
            for &i in discard {
                enabled[i] = false;
                let reason = if shared_segment {
                    "distinct arcs share a segment"
                } else {
                    "distinct arcs cross"
                };
                eprintln!("{prefix} id={} unavailable: {reason} at fine grid near {at:?} (other IDs {kept:?})", prepared[i].id.0);
            }
            retry = true;
        }
        if retry {
            continue;
        }
        // All shared arcs and endpoints have been tried at fine precision.
        // Keep the greater entity extent; equal extents favor the lower ID.
        for [left, right] in overlaps {
            if enabled[left] && enabled[right] {
                let (omit, keep) = if extent(left) < extent(right) {
                    (left, right)
                } else {
                    (right, left)
                };
                enabled[omit] = false;
                eprintln!("{prefix} id={} unavailable: new component interior overlap at fine grid (other ID {})", prepared[omit].id.0, prepared[keep].id.0);
                retry = true;
            }
        }
        if retry {
            continue;
        }
        for (&i, reason) in &fallback {
            eprintln!(
                "{prefix} id={} fallback to unsimplified/fine: {reason}",
                prepared[i].id.0
            );
        }
        let records: Vec<_> = prepared
            .iter()
            .zip(shapes)
            .filter_map(|(s, shape)| {
                shape.map(|polygons| DisplayRecord {
                    worldview,
                    entity_id: s.id,
                    bounds: s.bounds,
                    polygons: polygons.unwrap(),
                })
            })
            .collect();
        let repaired = prepared.iter().filter(|s| s.repaired).count();
        let fine_arcs = graph.arcs.iter().filter(|a| a.fine).count();
        eprintln!("{prefix} {} records / {} sources; {} fallback requests, {fine_arcs} fine arcs, {repaired} repaired, {} unavailable", records.len(), source_count, fallback.len(), source_count - records.len());
        return records;
    }
}
