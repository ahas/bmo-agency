//! The front's routes: each answered once, the session contract's and the boundary's own among them.

use agency_server::control::{moved_routes, route_faults, route_name};

#[test]
fn every_route_is_answered_once() {
  assert_eq!(route_faults(), Vec::<String>::new());
}

#[test]
fn the_boundary_s_routes_are_linked_in() {
  let names: Vec<String> = moved_routes().map(|route| route_name(&route.method(), route.path())).collect();

  for wanted in [
    "POST /session/start",
    "POST /session/turn",
    "POST /session/push",
    "GET /session/state",
    "POST /session/interrupt",
    "POST /session/fold",
    "POST /session/end",
    "GET /session/list",
    "GET /events",
    "POST /events/publish",
    "POST /hooks",
    "POST /hooks/remove",
    "GET /hooks",
    "POST /typed-line",
    "GET /road-verbs",
    "GET /stand-in",
    "POST /documents/reload",
    "GET /health",
    "POST /rooms/forget",
    "GET /run-log",
    "GET /run-records",
    "POST /jobs/stop-mark",
    "GET /roads/open",
    "POST /runs/stop",
    "POST /runs/alive",
    "GET /runs/clock",
    "POST /runs",
    "GET /runs/{id}",
    "POST /runs/{id}/close",
    "GET /runs",
    "POST /runs/{id}/launch",
    "GET /runs/overtime",
    "POST /runs/overtime",
    "GET /editors/cap",
    "GET /rulebooks",
    "GET /rest",
    "POST /rest",
    "GET /rest/passed",
    "GET /reset-point",
    "POST /companions/add",
    "POST /companions/remove",
    "POST /pool/enqueue",
    "POST /pool/acquire",
    "POST /pool/reserve",
    "POST /places/take",
    "POST /workers/stand",
  ] {
    assert!(names.iter().any(|name| name == wanted), "{wanted} is not linked in");
  }
}
