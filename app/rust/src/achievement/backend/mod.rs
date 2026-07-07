//! The achievement-store backend: the compute-on-demand
//! [`on_demand::OnDemandStore`] (no persistence — holds only the active geo,
//! recomputes each read from the snapshot).
pub mod on_demand;
