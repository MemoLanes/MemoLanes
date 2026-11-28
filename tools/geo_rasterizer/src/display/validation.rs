//! Materialize and check each snapshot before committing display outlines.

mod contacts;
mod overlaps;

use std::collections::BTreeMap;

use anyhow::{bail, ensure, Result};
use geo::{winding_order::Winding, Area, BooleanOps, Euclidean, Length};
use geo_data_format::{DisplayPolygons, DisplayRecord, Worldview};
use geo_types::{LineString, MultiPolygon};
use rayon::prelude::*;

use super::{
    rings,
    source::{valid_geometry, Prepared},
    topology::{polygon_from_vertices, ArcState, Graph, Precision, Shape, Vertex},
};
use contacts::SourceContacts;
use overlaps::changed_nesting;

fn validated_shape(
    graph: &Graph,
    owner: usize,
    states: &[ArcState],
    source: &Prepared,
) -> Result<Shape> {
    let mut shape = vec![];
    let budget = source.budget;
    for (ring_ids, reference) in graph.shapes[owner].iter().zip(&source.geometry.0) {
        let mut polygon = vec![];
        for (&r, original) in ring_ids.iter().zip(rings(reference)) {
            let mut ring = vec![];
            for &(a, forward) in &graph.rings[r].arcs {
                let state = &states[a];
                ensure!(
                    state.error <= budget,
                    "ring {r} arc {a} fit error {:.7}° > {:.7}°",
                    state.error,
                    budget
                );
                let path = &state.points;
                if forward {
                    ring.extend(path[..path.len() - 1].iter().copied());
                } else {
                    ring.extend(path[1..].iter().rev().copied());
                }
            }
            ring.dedup_by(|a, b| a.degrees() == b.degrees());
            if ring
                .last()
                .zip(ring.first())
                .is_some_and(|(a, b)| a.degrees() != b.degrees())
            {
                ring.push(ring[0]);
            }
            ensure!(ring.len() >= 4, "ring {r} collapsed");
            let snapped = LineString(ring.iter().copied().map(Vertex::degrees).collect());
            ensure!(
                snapped.winding_order().is_some()
                    && snapped.winding_order() == original.winding_order(),
                "ring {r} collapsed or changed winding"
            );
            polygon.push(ring);
        }
        shape.push(polygon);
    }
    let candidate = MultiPolygon(
        shape
            .iter()
            .map(|rings| polygon_from_vertices(rings))
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
    prefix: &str,
) -> Result<Vec<DisplayRecord>> {
    let mut graph = Graph::build(prepared);
    let mut fallback = BTreeMap::new();
    let source_contacts = SourceContacts::new(&graph);
    graph.simplify(prepared, &source_contacts.protected_edges);
    loop {
        let states: Vec<_> = graph
            .arcs
            .par_iter()
            .map(|a| ArcState::new(&graph, a))
            .collect();
        let shapes: Vec<_> = prepared
            .par_iter()
            .enumerate()
            .map(|(i, s)| validated_shape(&graph, i, &states, s))
            .collect();
        let failures = shapes
            .iter()
            .enumerate()
            .filter_map(|(i, shape)| shape.as_ref().err().map(|error| (i, error)))
            .collect::<Vec<_>>();
        let mut retry = false;
        for &(i, error) in &failures {
            if graph.back_off_shape(i, Precision::Fine) {
                fallback.entry(i).or_insert_with(|| format!("{error:#}"));
                retry = true;
            }
        }
        // All failures refer to the same snapshot. A preceding request may
        // already have refined this shape's shared arcs or endpoints.
        if retry {
            continue;
        }
        if !failures.is_empty() {
            for &(i, _) in &failures {
                retry |= graph.back_off_shape(i, Precision::Exact);
            }
            if retry {
                continue;
            }
            let (i, error) = failures[0];
            bail!(
                "{prefix} id={} invalid at source precision: {error:#}",
                prepared[i].id.0
            );
        }
        let conflicts = source_contacts.conflicts(&graph, &states);
        for precision in [Precision::Fine, Precision::Exact] {
            for conflict in &conflicts {
                for arc in conflict.arcs {
                    if graph.back_off_arc(arc, precision) {
                        for &owner in &graph.arcs[arc].owners {
                            fallback
                                .entry(owner)
                                .or_insert_with(|| conflict.reason.to_owned());
                        }
                        retry = true;
                    }
                }
            }
            if retry {
                break;
            }
        }
        if retry {
            continue;
        }
        if let Some(conflict) = conflicts.first() {
            let ids = conflict.arcs.map(|arc| {
                graph.arcs[arc]
                    .owners
                    .iter()
                    .map(|&i| prepared[i].id.0)
                    .collect::<Vec<_>>()
            });
            bail!(
                "{prefix} {} at source precision near {:?} (arcs {:?}, IDs {ids:?})",
                conflict.reason,
                conflict.at,
                conflict.arcs
            );
        }

        let shapes = shapes.into_iter().collect::<Result<Vec<_>>>()?;
        let nesting = changed_nesting(&graph, prepared, &shapes);
        for precision in [Precision::Fine, Precision::Exact] {
            for change in &nesting {
                let [left, right] = change.rings;
                for (ring, other) in [(left, right), (right, left)] {
                    if graph.back_off_ring(ring, precision) {
                        let i = graph.rings[ring].owner;
                        let other = graph.rings[other].owner;
                        fallback.entry(i).or_insert_with(|| {
                            format!(
                                "changed ring containment (other ID {})",
                                prepared[other].id.0
                            )
                        });
                        retry = true;
                    }
                }
            }
            if retry {
                break;
            }
        }
        if retry {
            continue;
        }
        if let Some(change) = nesting.first() {
            let ids = change
                .rings
                .map(|ring| prepared[graph.rings[ring].owner].id.0);
            bail!("{prefix} changed ring containment at source precision near {:?} (rings {:?}, IDs {ids:?})", change.at, change.rings);
        }
        for (&i, reason) in &fallback {
            eprintln!(
                "{prefix} id={} restored boundary precision: {reason}",
                prepared[i].id.0
            );
        }
        let records: Vec<DisplayRecord> = prepared
            .iter()
            .zip(shapes)
            .map(|(s, shape)| DisplayRecord {
                worldview,
                entity_id: s.id,
                bounds: s.bounds,
                polygons: display_polygons(shape),
            })
            .collect();
        let repaired = prepared.iter().filter(|s| s.repaired).count();
        let fine_arcs = graph
            .arcs
            .iter()
            .filter(|a| a.precision == Precision::Fine)
            .count();
        let exact_arcs = graph
            .arcs
            .iter()
            .filter(|a| a.precision == Precision::Exact)
            .count();
        let exact_records = records
            .iter()
            .filter(|r| matches!(r.polygons, DisplayPolygons::Exact(_)))
            .count();
        eprintln!("{prefix} {} records ({exact_records} source-precision frames), {} owners retried, {fine_arcs} fine arcs, {exact_arcs} exact arcs, {repaired} repaired", records.len(), fallback.len());
        return Ok(records);
    }
}

fn display_polygons(shape: Shape) -> DisplayPolygons {
    fn map<T>(shape: Shape, vertex: impl Fn(Vertex) -> T) -> Vec<Vec<Vec<T>>> {
        shape
            .into_iter()
            .map(|p| {
                p.into_iter()
                    .map(|r| r.into_iter().map(&vertex).collect())
                    .collect()
            })
            .collect()
    }
    if shape.iter().flatten().flatten().all(|v| v.grid().is_some()) {
        DisplayPolygons::Grid(map(shape, |v| v.grid().unwrap()))
    } else {
        DisplayPolygons::Exact(map(shape, |v| {
            let p = v.degrees();
            [p.x, p.y]
        }))
    }
}
