//! Research runs (docs/research-runs.md): the research keys and their settings, the variables a run carries into
//! its tool session's record, a research key's road on the one resolution — Claude Code alone carries the
//! search line, and a closed road says so — and the routes refusing what is no research or what nothing can
//! carry now.

use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::control::harness::{OrderEntry, set_provider_order};
use agency_server::lines::token::research_record;
use agency_server::now_ms;
use agency_server::places::Ground;
use agency_server::research::*;
use agency_server::road::{Ask, resolve};
use agency_server::seeds::deployment;
use bmo_agency_client::{ResearchKeySettings, ResearchRunAsk, ResearchSettingsChange};
use harness_providers::{ARCHIVE_PROVIDERS, Machine};
use serde_json::{Value, json};

use crate::scratch::Scratch;
use crate::standing;

#[test]
fn every_research_key_stands_on_its_line_with_the_policy_s_settings() {
  let keys: Vec<(&str, &str)> =
    research_keys().map(|profile| (profile.key.as_str(), profile.line.as_str())).collect();

  assert_eq!(
    keys,
    [
      ("search:basic", "search"),
      ("search:thorough", "search"),
      ("search:exhaustive", "search"),
      ("browse", "browse")
    ]
  );
  assert!(research_profile("job-worker").is_none() && research_profile("search").is_none());
  assert_eq!(defaults().keys.len(), 4);
}

#[test]
fn a_change_lays_over_the_settings_and_one_that_cannot_stand_is_refused_whole() {
  let change = ResearchSettingsChange {
    strategy: Some("browser-first".into()),
    host_interval_ms: Some(Some(400)),
    keys: Some(vec![ResearchKeySettings {
      key: "search:thorough".into(),
      branches: 3,
      minutes: 20,
      pages: None,
      branch_pages: Some(5),
    }]),
    ..ResearchSettingsChange::default()
  };
  let laid = changed(defaults(), &change).unwrap();

  assert_eq!(laid.strategy, "browser-first");
  assert_eq!(laid.host_interval_ms, Some(400));
  assert_eq!(
    laid.keys["search:thorough"],
    Kept {
      branches: 3,
      minutes: 20,
      pages: Some(8),
      branch_pages: Some(5),
    },
    "a budget left out keeps the key's own"
  );
  assert_eq!(
    laid.keys["search:basic"],
    defaults().keys["search:basic"],
    "a key left out keeps its own"
  );

  let back = ResearchSettingsChange {
    host_interval_ms: Some(None),
    ..ResearchSettingsChange::default()
  };

  assert_eq!(
    changed_ok(laid, &back).host_interval_ms,
    None,
    "a gap given as null is the web tools' again"
  );

  for wrong in [
    json!({ "strategy": "fast" }),
    json!({ "hostIntervalMs": -1 }),
    json!({ "branchRole": "nobody" }),
    json!({ "keys": [{ "key": "job-worker", "branches": 1, "minutes": 5 }] }),
    json!({ "keys": [{ "key": "browse", "branches": 0, "minutes": 0 }] }),
    json!({ "keys": [{ "key": "search:basic", "branches": 0, "minutes": 5, "pages": 0 }] }),
  ] {
    let change: ResearchSettingsChange = serde_json::from_value(wrong.clone()).unwrap();

    assert!(changed(defaults(), &change).is_err(), "{wrong}");
  }
}

fn changed_ok(settings: Settings, change: &ResearchSettingsChange) -> Settings {
  changed(settings, change).unwrap()
}

#[test]
fn a_run_s_settings_travel_on_its_tool_session_record() {
  let mut settings = defaults();

  settings.engine_interval_ms = Some(1500);
  settings.keys.insert(
    "search:thorough".into(),
    Kept {
      branches: 2,
      minutes: 15,
      pages: Some(8),
      branch_pages: Some(4),
    },
  );

  let vars: std::collections::BTreeMap<String, String> =
    variables(&settings, "search:thorough", "search", Some("the owner"), Some(-5), 1_900_000_000_000);
  let (record, withheld) = research_record(&vars);
  let record = record.expect("a research run carries its settings");

  assert_eq!(record["closesAt"], 1_900_000_000_000_i64);
  assert_eq!(record["branches"], 2);
  assert_eq!(record["role"], json!(settings.branch_role));
  assert_eq!(record["source"], "search");
  assert_eq!(record["strategy"], json!(settings.strategy));
  assert_eq!(record["engineIntervalMs"], 1500);
  assert!(record.get("hostIntervalMs").is_none(), "a gap at the web tools' default is not named");
  assert!(!withheld.contains(&"web.branches"), "a run that may branch is shown the branch tool");

  let none = variables(&defaults(), "search:basic", "search", None, None, 1);

  assert!(research_record(&none).1.contains(&"web.branches"), "a run of no branches is not");
}

#[test]
fn a_branch_s_record_names_its_parent_and_holds_it_to_one_branch_s_budget() {
  let mut settings = defaults();

  settings.keys.insert(
    "search:thorough".into(),
    Kept {
      branches: 2,
      minutes: 15,
      pages: Some(8),
      branch_pages: Some(4),
    },
  );

  let mut vars: std::collections::BTreeMap<String, String> =
    variables(&settings, "search:thorough", "search", Some("the owner"), Some(-5), 1_900_000_000_000);

  vars.insert("BMO_RESEARCH_BRANCH_OF".into(), "6191".into());

  let (record, withheld) = research_record(&vars);
  let record = record.expect("a branch carries its parent's settings");

  assert_eq!(record["branchOf"], 6191);
  assert_eq!(record["pages"], 4, "a branch is held to the budget each branch of its parent has");
  assert!(record.get("branchPages").is_none(), "a branch opens no branch to give a budget");
  assert_eq!(record["branches"], 0);
  assert_eq!(record["closesAt"], 1_900_000_000_000_i64);
  assert!(withheld.contains(&"web.branches"), "a branch is not shown the branch tool");

  vars.remove("BMO_RESEARCH_BRANCH_OF");

  let parent = research_record(&vars).0.unwrap();

  assert!(parent.get("branchOf").is_none());
  assert_eq!(parent["pages"], 8);
  assert_eq!(parent["branchPages"], 4);
}

#[test]
fn an_ask_names_a_research_key_and_a_question() {
  let ask = |key: &str, query: &str| ResearchRunAsk {
    key: key.into(),
    query: query.into(),
    ..ResearchRunAsk::default()
  };

  assert!(ask_fault(&ask("search:basic", "what is open now")).is_none());
  assert!(ask_fault(&ask("job-worker", "what")).is_some());
  assert!(ask_fault(&ask("browse", "  ")).is_some());
}

fn ground_on(scratch: &Scratch, home: &std::path::Path) -> Ground {
  let variables: HashMap<String, String> = HashMap::new();
  let service =
    Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());
  let data = home.join("data");
  let env_dir = data.join("env");

  std::fs::create_dir_all(&env_dir).unwrap();
  for (name, key) in [("llm-gateway.key", "a key\n"), ("deepseek.key", "a key\n")] {
    let file = env_dir.join(name);

    std::fs::write(&file, key).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
  }

  let members = data.join("llm-gateway");

  std::fs::create_dir_all(&members).unwrap();
  std::fs::write(
    members.join("members.json"),
    r#"[{"name":"deepseek-flash","road":"deepseek","provider":"deepseek","model":"deepseek-flash","api_base":"https://api.deepseek.com/v1","key_file":"deepseek.key","wait_seconds":300,"context_tokens":1048576,"answer_tokens":32768,"thinking":true}]"#,
  )
  .unwrap();

  let env = vec![
    ("HOME".to_owned(), home.to_string_lossy().into_owned()),
    ("PATH".to_owned(), "/nowhere".to_owned()),
  ];

  Ground::new(&service, &Machine::with_env(env, data), None)
}

async fn every_on(scratch: &Scratch) {
  let order: Vec<OrderEntry> = ARCHIVE_PROVIDERS
    .iter()
    .map(|name| OrderEntry {
      name: (*name).to_owned(),
      enabled: true,
    })
    .collect();

  set_provider_order(scratch.db(), &order).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_search_key_goes_out_on_a_program_that_carries_the_line_and_its_closed_road_says_why() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(&scratch, home.path());

  store::seed::settle(scratch.db(), &deployment(&ground.machine())).await.unwrap();
  every_on(&scratch).await;
  // DeepSeek at the head of the walk, so its program's carrying the search line is what the run takes.
  let order: Vec<OrderEntry> = ["deepseek", "claude-code", "codex", "openrouter", "registered"]
    .iter()
    .map(|name| OrderEntry {
      name: (*name).to_owned(),
      enabled: true,
    })
    .collect();

  set_provider_order(scratch.db(), &order).await.unwrap();

  let open = resolve(&ground, &Ask::key("search:thorough")).await.unwrap();

  assert_eq!(open.road().map(|road| road.provider.as_str()), Some("deepseek"), "{open:?}");
  // DeepSeek's program carries the search line, so it is a step of the walk; Codex's does not.
  assert!(!open.uncarried.contains(&"deepseek".to_owned()), "{open:?}");
  assert!(open.uncarried.contains(&"codex".to_owned()), "{open:?}");

  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") SELECT 'rest_until:' || "id", $1 FROM "claude_accounts""#,
      vec![(now_ms() / 1000 + 3600).to_string().into()],
    )
    .await;

  // With the Claude accounts resting, DeepSeek turned off closes the last road that carries the line.
  let order: Vec<OrderEntry> = ["deepseek", "claude-code", "codex", "openrouter", "registered"]
    .iter()
    .map(|name| OrderEntry {
      name: (*name).to_owned(),
      enabled: *name != "deepseek",
    })
    .collect();

  set_provider_order(scratch.db(), &order).await.unwrap();

  let closed = resolve(&ground, &Ask::key("search:thorough")).await.unwrap();
  let sentence = closed.sentence();

  assert_eq!(closed.road(), None);
  assert!(sentence.contains("carries the search line"), "{sentence}");
  assert!(sentence.contains("tool-gate"), "{sentence}");
}

async fn call(
  port: u16,
  token: &str,
  method: reqwest::Method,
  path: &str,
  body: Option<Value>,
) -> (u16, Value) {
  let mut asked = reqwest::Client::new()
    .request(method, format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token);

  if let Some(body) = body {
    asked = asked.json(&body);
  }

  let answer = asked.timeout(Duration::from_secs(60)).send().await.unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

#[tokio::test(flavor = "multi_thread")]
async fn the_routes_keep_the_settings_and_refuse_what_is_no_research_or_what_nothing_can_carry() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ('rest_until:default', $1)"#,
      vec![(now_ms() / 1000 + 3600).to_string().into()],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;
  let (status, read) = call(port, token, reqwest::Method::GET, "/research-settings", None).await;

  assert_eq!(status, 200, "{read}");
  assert_eq!(read["source"], "default");
  assert_eq!(read["keys"].as_array().map(Vec::len), Some(4));
  assert!(
    read["engines"].is_null() && read["enginesUnread"].is_string(),
    "no tool host here: {read}"
  );

  let (status, set) = call(
    port,
    token,
    reqwest::Method::PUT,
    "/research-settings",
    Some(json!({ "strategy": "direct-only", "keys": [{ "key": "browse", "branches": 0, "minutes": 9 }] })),
  )
  .await;

  assert_eq!(status, 200, "{set}");
  assert_eq!((&set["source"], &set["strategy"]), (&json!("set"), &json!("direct-only")));
  assert_eq!(
    call(
      port,
      token,
      reqwest::Method::PUT,
      "/research-settings",
      Some(json!({ "strategy": "fast" }))
    )
    .await
    .0,
    400
  );
  assert_eq!(
    call(port, token, reqwest::Method::GET, "/research-settings", None).await.1["strategy"],
    "direct-only",
    "a refused change wrote nothing"
  );

  let (status, refused) = call(
    port,
    token,
    reqwest::Method::POST,
    "/research-runs",
    Some(json!({ "key": "job-worker", "query": "anything" })),
  )
  .await;

  assert_eq!((status, refused["code"].as_str()), (400, Some("bad-request")), "{refused}");

  // The one login rests and no other provider's program carries the search line: nothing is launched.
  let (status, closed) = call(
    port,
    token,
    reqwest::Method::POST,
    "/research-runs",
    Some(json!({ "key": "search:basic", "query": "what is open now" })),
  )
  .await;

  assert_eq!((status, closed["code"].as_str()), (503, Some("no-login-open")), "{closed}");
  assert!(closed["error"].as_str().unwrap_or_default().contains("search line"), "{closed}");
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "runs""#, vec![]).await, 0);

  let (_, fallback) = call(port, token, reqwest::Method::GET, "/fallback", None).await;
  let lines: Vec<&str> = fallback["profiles"]
    .as_array()
    .unwrap()
    .iter()
    .filter_map(|profile| profile["line"].as_str())
    .collect();

  assert_eq!(
    lines.iter().filter(|line| **line == "search").count(),
    3,
    "the console's research tab fills"
  );
  assert_eq!(lines.iter().filter(|line| **line == "browse").count(), 1);
}

// The rounds go out on a session of their own whose record withholds the branches, the image viewer and
// the rulebook reader (docs/launch-lines.md, 「A research run's rounds」).
#[test]
fn the_rounds_hold_the_reading_tools_alone() {
  let vars =
    variables(&defaults(), "search:thorough", "search", Some("the owner"), Some(5), 1_900_000_000_000);
  let rounds = rounds_variables(&vars);

  assert!(vars.contains_key("BMO_RESEARCH_TOOLS"));
  assert!(!rounds.contains_key("BMO_RESEARCH_TOOLS") && !rounds.contains_key("BMO_RESEARCH_IMAGES"));
  assert!(!rounds.contains_key("BMO_RESEARCH_RULEBOOKS"));
  assert_eq!(rounds["BMO_RESEARCH_BRANCHES"], "0");
  assert_eq!(rounds["BMO_RESEARCH_CLOSES_AT"], vars["BMO_RESEARCH_CLOSES_AT"]);
}

// Each depth's budget is the policy's and is told to the run as a ceiling, and travels on its record
// (docs/research-runs.md, 「What a run is held to: the research settings」).
#[test]
fn each_depth_holds_its_budget_and_the_run_is_told_it_as_a_ceiling() {
  let keys = defaults().keys;
  let budget = |key: &str| (keys[key].pages, keys[key].branches, keys[key].branch_pages);

  assert_eq!(budget("search:basic"), (Some(3), 0, None));
  assert_eq!(budget("search:thorough"), (Some(8), 2, Some(4)));
  assert_eq!(budget("search:exhaustive"), (Some(20), 5, Some(6)));
  assert_eq!(keys["browse"].pages, None, "a browse names no budget");

  let told = budget_line(keys["search:thorough"]);

  assert!(
    told.contains("at most 8 pages") && told.contains("each of the 2 branches it may open at most 4"),
    "{told}"
  );
  assert!(told.contains("fetch nothing more") && told.contains("UNCONFIRMED"), "{told}");
  assert!(!budget_line(keys["search:basic"]).contains("branches"));
  assert_eq!(budget_line(keys["browse"]), "");

  let vars = variables(&defaults(), "search:thorough", "search", None, None, 1);
  let (record, _) = research_record(&vars);
  let record = record.unwrap();

  assert_eq!((record["pages"].as_i64(), record["branchPages"].as_i64()), (Some(8), Some(4)));
}

#[test]
fn a_research_run_s_place_is_its_own_by_its_record_and_none_without_it() {
  let data = tempfile::tempdir().unwrap();
  let place = data.path().join("agent-places").join("search-7");

  assert!(!is_run_place(data.path(), "search-7"));

  std::fs::create_dir_all(&place).unwrap();
  std::fs::write(place.join(RUN_PLACE), json!({ "place": "search-7", "run": 7 }).to_string()).unwrap();
  std::fs::create_dir_all(data.path().join("agent-places").join("a-worker")).unwrap();

  assert!(is_run_place(data.path(), "search-7"));
  assert_eq!(run_places(data.path()), ["search-7"]);
}
