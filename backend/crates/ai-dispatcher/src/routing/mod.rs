//! `routing` — Stage E route selection + the data-export guard (`compliance/02`).
//!
//! Two layers live here, both owned by `ai-dispatcher` (the唯一调用方 of the guard, `compliance/02`
//! §8; D6):
//!
//! - [`decide`] — the Stage E degrade-level route selector (`decide_route` / [`RouteTarget`],
//!   ai/01 §1.5 Stage A + E): HSD `ForceLocal` pins to local, otherwise the route follows the
//!   Level0-4 degrade FSM.
//! - [`decision`] / [`data_export_guard`] / [`desensitize`] / [`audit_writer`] — the 数据出境守卫
//!   (`compliance/02` §3): the §3.1 routing matrix ([`route`]/[`GuardRequest`] →
//!   [`RoutingDecision`]), the domestic-cloud 脱敏管线 ([`desensitize`]), and the §6 audit
//!   four-tuple writer ([`write_routing_audit`] over the [`RoutingAuditSink`] adapter, INV-06).
//!
//! The guard is the only place that may emit [`decision::Route::OverseasCloud`], and only with a
//! second confirm required (§4.4); L4 / HSD-hit / forced-local scenes always end at local (INV-05).

pub mod audit_writer;
pub mod data_export_guard;
pub mod decide;
pub mod decision;
pub mod desensitize;

// Stage E route selection (re-exported at the crate root via `pub use routing::{decide_route,
// RouteTarget}` in lib.rs; `stage.rs` imports them through `crate::routing`).
pub use decide::{decide_route, RouteTarget};

// Data-export guard surface (`compliance/02` §3 — the INV-05 boundary).
pub use audit_writer::{write_routing_audit, RoutingAuditEntry, RoutingAuditSink};
pub use data_export_guard::{max_grade_of_fields, route, ForcedLocalScene, GuardRequest};
pub use decision::{Route, RoutingDecision};
pub use desensitize::{desensitize, Desensitized};
