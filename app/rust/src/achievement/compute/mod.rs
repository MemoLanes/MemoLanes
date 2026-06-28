//! Pure computation kernels: snapshot (+ geo) → state. Backend-agnostic and
//! side-effect free — the on-demand reader calls these to derive its values.
pub mod explored_area;
pub mod region_state;
