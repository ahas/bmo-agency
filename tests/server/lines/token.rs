use std::collections::BTreeMap;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::lines::token::*;
use agency_server::places::Ground;
use harness_providers::Machine;
use serde_json::json;

fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
  pairs.iter().map(|(name, value)| ((*name).to_owned(), (*value).to_owned())).collect()
}

#[test]
fn a_run_with_no_research_setting_is_opened_no_research_tool() {
  assert_eq!(
    research_record(&env(&[("BMO_RESEARCH_TOOLS", "")])),
    (None, vec!["web.branches", "web.images", "web.rulebooks"])
  );
}

#[test]
fn a_branch_line_ping_is_opened_the_picture_viewer_alone() {
  let (research, withheld) = research_record(&env(&[("BMO_RESEARCH_IMAGES", "1")]));

  assert_eq!(research, Some(json!({ "branches": 0, "chat": 0, "rulebooks": [] })));
  assert_eq!(withheld, vec!["web.branches", "web.rulebooks"]);
}

#[test]
fn a_research_run_carries_what_its_runner_told_it() {
  let (research, withheld) = research_record(&env(&[
    ("BMO_RESEARCH_TOOLS", "1"),
    ("BMO_RESEARCH_RULEBOOKS", "a,,b"),
    ("BMO_RESEARCH_CLOSES_AT", "1700"),
    ("BMO_RESEARCH_SOURCE", "elsewhere"),
    ("BMO_RESEARCH_OWNER", "research:7"),
    ("BMO_RESEARCH_BRANCHES", "3"),
    ("BMO_RESEARCH_CHAT", "-42x"),
    ("BMO_WEB_HOST_INTERVAL_MS", "1.5"),
  ]));

  assert_eq!(
    research,
    Some(
      json!({ "closesAt": 1700, "owner": "research:7", "branches": 3, "chat": -42, "rulebooks": ["a", "b"] })
    )
  );
  assert_eq!(withheld, vec!["web.images"]);
}

// The paragraph a stand-in tool host words, which no agency text holds.
pub(crate) const PARAGRAPH: &str = "Reach a tool with bmo use: list, describe, then call.";

/// A tool host standing on `socket` that answers `GET /discipline` as text to the issuer `token` alone.
pub(crate) fn tool_host(socket: &std::path::Path, token: &'static str) {
  let listener = tokio::net::UnixListener::bind(socket).unwrap();
  let app = axum::Router::new().route(
    "/discipline",
    axum::routing::get(move |headers: axum::http::HeaderMap| async move {
      let bearer = headers.get("authorization").and_then(|value| value.to_str().ok());

      if bearer == Some(format!("Bearer {token}").as_str()) {
        (
          axum::http::StatusCode::OK,
          [("content-type", "text/plain; charset=utf-8")],
          PARAGRAPH.to_owned(),
        )
      } else {
        (
          axum::http::StatusCode::UNAUTHORIZED,
          [("content-type", "application/json")],
          json!({ "error": "the token is not known", "code": "unauthorized" }).to_string(),
        )
      }
    }),
  );

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
}

// A ground whose tool host is the socket named, asked with the issuer token named.
fn ground_with_host(home: &std::path::Path, socket: &std::path::Path, issuer: &str) -> Ground {
  let variables: std::collections::HashMap<String, String> = std::collections::HashMap::new();
  let service = Service::new(
    sea_orm::DatabaseConnection::Disconnected,
    Config::read(|name| variables.get(name).cloned()).unwrap(),
  );
  let machine = Machine::with_env(
    [
      ("HOME".to_owned(), home.to_string_lossy().into_owned()),
      ("BMO_MCP_HOST_SOCKET".to_owned(), socket.to_string_lossy().into_owned()),
      ("BMO_MCP_HOST_ISSUER_TOKEN".to_owned(), issuer.to_owned()),
    ],
    home.join("data"),
  );

  Ground::new(&service, &machine, None)
}

#[tokio::test]
async fn the_tool_hosts_discipline_is_read_whole_with_the_issuer_token() {
  let home = tempfile::tempdir().unwrap();
  let socket = home.path().join("mcp-host.sock");

  tool_host(&socket, "issuer");

  assert_eq!(
    discipline(&ground_with_host(home.path(), &socket, "issuer")).await.as_deref(),
    Ok(PARAGRAPH)
  );

  let refused = discipline(&ground_with_host(home.path(), &socket, "someone")).await.unwrap_err();

  assert!(refused.contains("401"), "{refused}");
}

#[tokio::test]
async fn a_tool_host_that_does_not_answer_gives_no_discipline_and_says_where_it_was_asked() {
  let home = tempfile::tempdir().unwrap();
  let socket = home.path().join("nobody.sock");
  let why = discipline(&ground_with_host(home.path(), &socket, "issuer")).await.unwrap_err();

  assert!(why.contains(&socket.to_string_lossy().into_owned()), "{why}");
}

// The chat line names the gate's address and token file for the translate tool's record; a variable the
// deployment leaves unset or empty is handed nothing, and a line that names none hands none.
#[test]
fn a_line_hands_its_record_the_deployment_variables_it_names_where_they_are_set() {
  let home = tempfile::tempdir().unwrap();
  let variables: std::collections::HashMap<String, String> = std::collections::HashMap::new();
  let service = Service::new(
    sea_orm::DatabaseConnection::Disconnected,
    Config::read(|name| variables.get(name).cloned()).unwrap(),
  );
  let machine = Machine::with_env(
    [
      ("BMO_GATE_URL".to_owned(), " http://127.0.0.1:9 ".to_owned()),
      ("BMO_GATE_TOKEN_FILE".to_owned(), String::new()),
      ("BMO_OTHER".to_owned(), "x".to_owned()),
    ],
    home.path().join("data"),
  );
  let ground = Ground::new(&service, &machine, None);

  assert_eq!(
    line_variables(&ground, "chat"),
    [("BMO_GATE_URL".to_owned(), "http://127.0.0.1:9".to_owned())]
  );
  assert!(line_variables(&ground, "worker").is_empty());
}
