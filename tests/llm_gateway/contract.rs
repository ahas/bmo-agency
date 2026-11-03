//! The model gateway's wire contract (docs/own-llm.md, 「The gateway」), asked of the command under test
//! as a run asks it: the gateway started as a process on a free port over a scratch data root, its
//! keys and a scratch store holding the registry, with a stand-in for LiteLLM first on its `PATH` that
//! answers as a child does and writes down how it was started, what it was asked and when it was
//! stopped; and once with LiteLLM itself, as installed for the account, in front of a backend of the
//! test's own. The command is the crate's binary, or the one `LLM_GATEWAY_CONTRACT_COMMAND` names.

use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper_util::rt::TokioIo;
use sea_orm_migration::MigratorTrait;
use serde_json::{Value, json};
use sqlx::postgres::PgConnection;
use sqlx::{Connection, Executor};
use store::migration::Migrator;
use tokio::net::TcpStream;

use crate::support::scratch::Scratch;

const GATEWAY_KEY: &str = "gateway-key-0123456789";

// LiteLLM, standing in: started as the gateway starts it, it writes down its table, port, arguments
// and environment; answers its liveliness path; echoes every other request; holds `/held` open until
// the test lets it go; ends at `/die`; and writes down its stop. A `fail-start` file beside it makes
// it end before it answers.
const STANDIN: &str = r#"
import { appendFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
const dir = dirname(import.meta.dir);
const args = process.argv.slice(2);
const at = (flag) => args[args.indexOf(flag) + 1];
const config = at("--config");
const table = /config-(\w+)\.yaml$/.exec(config)[1];
const log = (entry) => appendFileSync(join(dir, "children.log"), JSON.stringify({ table, ...entry }) + "\n");
if (existsSync(join(dir, "fail-start"))) {
  log({ event: "failed" });
  process.exit(3);
}
log({ event: "started", args, cwd: process.cwd(), env: process.env });
process.on("SIGTERM", () => {
  log({ event: "stopped" });
  process.exit(0);
});
Bun.serve({
  hostname: "127.0.0.1",
  port: Number(at("--port")),
  idleTimeout: 0,
  async fetch(request) {
    const url = new URL(request.url);
    if (url.pathname === "/health/liveliness") return new Response("I'm alive!");
    if (url.pathname === "/die") process.exit(1);
    if (url.pathname === "/held") {
      log({ event: "holding" });
      return new Response(new ReadableStream({
        async start(controller) {
          controller.enqueue(new TextEncoder().encode(`${table}-first;`));
          while (!existsSync(join(dir, `release-${table}`))) await Bun.sleep(20);
          controller.enqueue(new TextEncoder().encode(`${table}-second`));
          controller.close();
        },
      }));
    }
    const body = await request.text();
    return Response.json(
      { table, method: request.method, path: url.pathname, search: url.search, headers: Object.fromEntries(request.headers), body },
      { status: Number(request.headers.get("x-standin-status") ?? 200), headers: { "x-standin": table, "content-encoding": "identity" } },
    );
  },
});
"#;

// A registration served by two nodes.
const PAIR: &str = "
  INSERT INTO registered_models VALUES ('pair-0-1', 'Pair 0.1', 'pair-model', 32768, 4096, 0, 0, 1, 0);
  INSERT INTO registered_model_addresses VALUES ('pair-0-1', 'alpha', 1, 'http://127.0.0.1:18021/v1/chat/completions', 'alpha-llm.token', 0, NULL);
  INSERT INTO registered_model_addresses VALUES ('pair-0-1', 'bravo', 2, 'http://127.0.0.1:18023/v1/chat/completions', NULL, 0, NULL);
";

// A registration served by one address.
const SINGLE: &str = "
  INSERT INTO registered_models VALUES ('single', 'Single', 'single-model', 65536, 8192, 0, 0, 2, 0);
  INSERT INTO registered_model_addresses VALUES ('single', 'd-vex', 1, 'https://llm.example.com/v1/chat/completions', NULL, 0, NULL);
";

fn private(path: &Path, text: &str, mode: u32) {
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, text).unwrap();
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

fn command() -> Command {
  match std::env::var("LLM_GATEWAY_CONTRACT_COMMAND") {
    Ok(named) if !named.trim().is_empty() => {
      let mut words = named.split_whitespace();
      let mut command = Command::new(words.next().unwrap());
      command.args(words);
      command
    }
    _ => Command::new(crate::support::binary::built("llm-gateway", "llm-gateway")),
  }
}

struct World {
  dir: tempfile::TempDir,
  store: Scratch,
  gateway: Option<Child>,
  port: u16,
}

impl World {
  /// A scratch world: the gateway's key and a node's key in place, the stand-in first on the `PATH`,
  /// and a migrated store holding the registrations given.
  async fn new(registry: &str) -> World {
    World::over(Scratch::new().await, registry).await
  }

  async fn over(store: Scratch, registry: &str) -> World {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    private(&root.join("data/env/llm-gateway.key"), &format!("{GATEWAY_KEY}\n"), 0o600);
    private(&root.join("data/env/alpha-llm.token"), "alpha-key\n", 0o600);
    std::fs::create_dir_all(root.join("bin")).unwrap();
    std::fs::write(root.join("bin/standin.ts"), STANDIN).unwrap();
    let wrapper = format!("#!/bin/sh\nexec bun {} \"$@\"\n", root.join("bin/standin.ts").display());
    private(&root.join("bin/litellm"), &wrapper, 0o755);
    std::fs::write(root.join("children.log"), "").unwrap();
    let world = World {
      dir,
      store,
      gateway: None,
      port: 0,
    };
    world.write(registry).await;
    world
  }

  /// The world with LiteLLM itself in place of the stand-in, found as the gateway's unit finds it.
  fn on_litellm(self) -> World {
    std::fs::remove_file(self.root().join("bin/litellm")).unwrap();
    self
  }

  fn root(&self) -> &Path {
    self.dir.path()
  }

  fn folder(&self) -> PathBuf {
    self.root().join("data/llm-gateway")
  }

  async fn write(&self, statements: &str) {
    if statements.trim().is_empty() {
      return;
    }
    let mut store = PgConnection::connect(&self.store.address()).await.unwrap();
    store.execute(statements).await.unwrap();
    store.close().await.unwrap();
  }

  fn spawn(&self) -> Child {
    // The account's own programs come after the stand-in, as on the unit's path.
    let path = format!(
      "{}:{}/.local/bin:{}",
      self.root().join("bin").display(),
      std::env::var("HOME").unwrap_or_default(),
      std::env::var("PATH").unwrap_or_default()
    );
    let log = std::fs::File::create(self.root().join("gateway.log")).unwrap();
    command()
      .args(["--port", &self.port.to_string()])
      .env("PATH", path)
      .env("BMO_DATA_DIR", self.root().join("data"))
      .env("BMO_ARCHIVE_URL", self.store.address())
      .env("BMO_CODE_ROOT", env!("CARGO_MANIFEST_DIR"))
      .env("LLM_GATEWAY_REFRESH_MS", "100")
      .stdin(Stdio::null())
      .stdout(log.try_clone().unwrap())
      .stderr(log)
      .process_group(0)
      .spawn()
      .expect("the gateway starts")
  }

  /// The gateway started on a port of its own choosing, read off the line it names it in, so no
  /// other process can take the port between its choosing and its binding.
  async fn start(mut self) -> World {
    self.gateway = Some(self.spawn());
    let ready = Instant::now() + Duration::from_secs(20);
    self.port = loop {
      let heard = self.said().lines().find_map(|line| {
        line
          .strip_prefix("llm-gateway: serving on http://127.0.0.1:")?
          .trim()
          .parse::<u16>()
          .ok()
      });
      if let Some(port) = heard {
        break port;
      }
      assert!(Instant::now() < ready, "the gateway never listened; it said {}", self.said());
      tokio::time::sleep(Duration::from_millis(20)).await;
    };
    self
  }

  fn said(&self) -> String {
    std::fs::read_to_string(self.root().join("gateway.log")).unwrap_or_default()
  }

  /// What the stand-in children wrote down, in order.
  fn children(&self) -> Vec<Value> {
    std::fs::read_to_string(self.root().join("children.log"))
      .unwrap_or_default()
      .lines()
      .map(|line| serde_json::from_str(line).unwrap())
      .collect()
  }

  fn events(&self, event: &str) -> Vec<Value> {
    self.children().into_iter().filter(|entry| entry["event"] == event).collect()
  }

  fn status(&self) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(self.folder().join("status.json")).ok()?).ok()
  }

  /// The table the gateway serves once it serves one other than `not`.
  async fn serving(&self, not: Option<&str>) -> String {
    self.serving_within(not, Duration::from_secs(30)).await
  }

  async fn serving_within(&self, not: Option<&str>, bound: Duration) -> String {
    let mut seen = None;
    until_within(&format!("a table other than {not:?} to be served"), bound, || {
      seen = self.status().and_then(|status| status["serving"].as_str().map(str::to_owned));
      seen.is_some() && seen.as_deref() != not
    })
    .await;
    seen.unwrap()
  }

  /// A request as a caller of the gateway sends it: with the gateway's key as its Bearer unless the
  /// headers name an authorization of their own.
  async fn open(
    &self,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
  ) -> hyper::Response<Incoming> {
    let keyed = format!("Bearer {GATEWAY_KEY}");
    let mut headers = headers.to_vec();
    if !headers.iter().any(|(name, _)| name.eq_ignore_ascii_case("authorization")) {
      headers.push(("authorization", &keyed));
    }
    self.open_bare(method, path, &headers, body).await
  }

  /// A request carrying the headers named and nothing more.
  async fn open_bare(
    &self,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
  ) -> hyper::Response<Incoming> {
    let stream = TcpStream::connect(("127.0.0.1", self.port)).await.unwrap();
    let (mut sender, connection) =
      hyper::client::conn::http1::handshake(TokioIo::new(stream)).await.unwrap();
    tokio::spawn(connection);
    let mut request = hyper::Request::builder().method(method).uri(path).header("host", "127.0.0.1:18030");
    for (name, value) in headers {
      request = request.header(*name, *value);
    }
    sender
      .send_request(request.body(Full::new(Bytes::from(body.to_owned()))).unwrap())
      .await
      .unwrap()
  }

  async fn ask(
    &self,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
  ) -> (u16, hyper::HeaderMap, String) {
    read_whole(self.open(method, path, headers, body).await).await
  }

  /// Which child answered a plain request.
  async fn answered_by(&self) -> String {
    let (status, headers, body) = self.ask("GET", "/v1/models", &[], "").await;
    assert_eq!(status, 200, "{body}");
    headers["x-standin"].to_str().unwrap().to_owned()
  }

  fn terminate(&self) {
    unsafe {
      libc::kill(self.gateway.as_ref().unwrap().id() as i32, libc::SIGTERM);
    }
  }
}

async fn read_whole(answer: hyper::Response<Incoming>) -> (u16, hyper::HeaderMap, String) {
  let (status, headers) = (answer.status().as_u16(), answer.headers().clone());
  let body = answer.into_body().collect().await.unwrap().to_bytes();
  (status, headers, String::from_utf8_lossy(&body).into_owned())
}

impl Drop for World {
  fn drop(&mut self) {
    // The gateway and every child it started, as one process group.
    if let Some(gateway) = &mut self.gateway {
      unsafe {
        libc::kill(-(gateway.id() as i32), libc::SIGKILL);
      }
      let _ = gateway.wait();
    }
  }
}

async fn until(what: &str, holds: impl FnMut() -> bool) {
  until_within(what, Duration::from_secs(30), holds).await
}

async fn until_within(what: &str, bound: Duration, mut holds: impl FnMut() -> bool) {
  let bound = Instant::now() + bound;
  while !holds() {
    assert!(Instant::now() < bound, "waited in vain for {what}");
    tokio::time::sleep(Duration::from_millis(20)).await;
  }
}

#[tokio::test]
async fn without_its_own_key_the_gateway_does_not_stand() {
  for mode in [None, Some(0o644)] {
    let world = World::new(PAIR).await;
    let key = world.root().join("data/env/llm-gateway.key");
    match mode {
      None => std::fs::remove_file(&key).unwrap(),
      Some(mode) => std::fs::set_permissions(&key, std::fs::Permissions::from_mode(mode)).unwrap(),
    }
    let mut gateway = world.spawn();
    let bound = Instant::now() + Duration::from_secs(20);
    let ended = loop {
      if let Some(ended) = gateway.try_wait().unwrap() {
        break ended;
      }
      assert!(Instant::now() < bound, "the gateway stood without its key");
      tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert_eq!(ended.code(), Some(1));
    assert!(world.said().contains("llm-gateway.key"), "{}", world.said());
    assert!(world.children().is_empty());
  }
}

#[tokio::test]
async fn before_any_table_it_answers_503_and_serves_the_registry_once_it_stands() {
  let world = World::over(Scratch::blank().await, "").await.start().await;

  let (status, _, body) = world.ask("GET", "/v1/models", &[], "").await;
  assert_eq!((status, body.as_str()), (503, "the gateway has no table to serve yet"));

  Migrator::up(world.store.db(), None).await.unwrap();
  world.write(PAIR).await;
  let table = world.serving(None).await;
  assert_eq!(world.answered_by().await, table);
}

#[tokio::test]
async fn a_request_and_its_answer_pass_as_they_stand() {
  let world = World::new(PAIR).await.start().await;
  let table = world.serving(None).await;

  let (status, headers, body) = world
    .ask(
      "POST",
      "/v1/messages?beta=true",
      &[
        ("authorization", &format!("Bearer {GATEWAY_KEY}")),
        ("x-litellm-timeout", "600"),
        ("accept-encoding", "gzip"),
        ("keep-alive", "timeout=5"),
        ("x-standin-status", "201"),
      ],
      r#"{"model":"pair-0-1"}"#,
    )
    .await;
  assert_eq!(status, 201);
  assert_eq!(headers["x-standin"], table.as_str());
  assert!(headers.get("content-encoding").is_none());
  let asked: Value = serde_json::from_str(&body).unwrap();
  assert_eq!(
    (&asked["method"], &asked["path"], &asked["search"], &asked["body"]),
    (
      &json!("POST"),
      &json!("/v1/messages"),
      &json!("?beta=true"),
      &json!(r#"{"model":"pair-0-1"}"#)
    )
  );
  assert_eq!(asked["headers"]["authorization"], format!("Bearer {GATEWAY_KEY}"));
  assert_eq!(asked["headers"]["x-litellm-timeout"], "600");
  assert_eq!(asked["headers"]["accept-encoding"], "identity");
  assert!(asked["headers"].get("keep-alive").is_none());

  // A child's own failure comes back as it came.
  let (status, _, _) = world.ask("GET", "/v1/models", &[("x-standin-status", "500")], "").await;
  assert_eq!(status, 500);
}

#[tokio::test]
async fn the_table_is_written_private_beside_its_hooks_and_the_keys_reach_the_child_alone() {
  let world = World::new(PAIR).await;
  private(&world.folder().join("config-0000000000000000.yaml"), "left over", 0o600);
  let world = world.start().await;
  let table = world.serving(None).await;

  assert!(!world.folder().join("config-0000000000000000.yaml").exists());
  let written = world.folder().join(format!("config-{table}.yaml"));
  assert_eq!(std::fs::metadata(&written).unwrap().permissions().mode() & 0o777, 0o600);
  let config: Value = serde_json::from_str(&std::fs::read_to_string(&written).unwrap()).unwrap();
  let names: Vec<&str> = config["model_list"]
    .as_array()
    .unwrap()
    .iter()
    .map(|entry| entry["model_name"].as_str().unwrap())
    .collect();
  assert_eq!(names, ["pair-0-1", "pair-0-1@alpha", "pair-0-1", "pair-0-1@bravo"]);
  assert_eq!(
    config["model_list"][0]["litellm_params"]["api_key"],
    "os.environ/BMO_KEY_ALPHA_LLM_TOKEN"
  );
  for hook in ["thinking.py", "sampling.py", "reminders.py", "long_tools.py", "roles.py"] {
    assert!(world.folder().join(hook).exists(), "{hook} was not copied beside the table");
  }

  let started = world.events("started");
  assert_eq!(started.len(), 1);
  let child = &started[0];
  let port = world.status().unwrap();
  assert_eq!(port["starting"], Value::Null);
  assert_eq!(port["draining"], 0);
  assert_eq!(
    std::fs::metadata(world.folder().join("status.json")).unwrap().permissions().mode() & 0o777,
    0o600
  );
  let args: Vec<&str> = child["args"].as_array().unwrap().iter().map(|arg| arg.as_str().unwrap()).collect();
  assert_eq!(args[..2], ["--config", written.to_str().unwrap()]);
  assert_eq!(args[2..4], ["--host", "127.0.0.1"]);
  assert_eq!(args[6..], ["--num_workers", "1"]);
  assert_eq!(child["cwd"], world.folder().to_str().unwrap());
  assert_eq!(child["env"]["LLM_GATEWAY_KEY"], GATEWAY_KEY);
  assert_eq!(child["env"]["BMO_KEY_ALPHA_LLM_TOKEN"], "alpha-key");
  for kept in ["BMO_DATA_DIR", "BMO_ARCHIVE_URL", "LLM_GATEWAY_REFRESH_MS"] {
    assert!(child["env"].get(kept).is_none(), "the child was handed {kept}");
  }

  // Readings that change nothing start nothing.
  let read = Instant::now();
  while read.elapsed() < Duration::from_secs(1) {
    tokio::time::sleep(Duration::from_millis(100)).await;
  }
  assert_eq!(world.events("started").len(), 1);
}

#[tokio::test]
async fn a_new_table_takes_over_and_the_old_child_is_stopped_only_once_its_request_ends() {
  let world = World::new(PAIR).await.start().await;
  let first = world.serving(None).await;

  let held = world.open("GET", "/held", &[], "").await;
  let mut held = held.into_body();
  let part = held.frame().await.unwrap().unwrap().into_data().unwrap();
  assert_eq!(part, format!("{first}-first;").as_bytes());

  world.write(SINGLE).await;
  let second = world.serving(Some(&first)).await;
  assert_eq!(world.answered_by().await, second);
  assert!(world.events("stopped").is_empty(), "the old child was stopped while it held a request");
  assert_eq!(world.status().unwrap()["draining"], 1);

  std::fs::write(world.root().join(format!("release-{first}")), "").unwrap();
  let mut rest = Vec::new();
  while let Some(frame) = held.frame().await {
    if let Ok(data) = frame.unwrap().into_data() {
      rest.extend_from_slice(&data);
    }
  }
  assert_eq!(rest, format!("{first}-second").as_bytes());
  until("the old child to be stopped", || {
    world.events("stopped").iter().any(|entry| entry["table"] == first.as_str())
  })
  .await;
  until("nothing to be draining", || {
    world.status().is_some_and(|status| status["draining"] == 0)
  })
  .await;
  assert!(!world.events("stopped").iter().any(|entry| entry["table"] == second.as_str()));
  assert!(!world.folder().join(format!("config-{first}.yaml")).exists());
}

#[tokio::test]
async fn a_key_given_again_is_a_new_child_on_it() {
  let world = World::new(PAIR).await.start().await;
  let first = world.serving(None).await;

  private(&world.root().join("data/env/alpha-llm.token"), "alpha-key-given-again\n", 0o600);
  let second = world.serving(Some(&first)).await;

  let started = world.events("started");
  let child = started.iter().find(|entry| entry["table"] == second.as_str()).unwrap();
  assert_eq!(child["env"]["BMO_KEY_ALPHA_LLM_TOKEN"], "alpha-key-given-again");
  assert_eq!(world.answered_by().await, second);
  // The old child held nothing, so it is stopped at once.
  until("the old child to be stopped", || {
    world.events("stopped").iter().any(|entry| entry["table"] == first.as_str())
  })
  .await;
}

// A refusal for want of the key, as the front answers it: 401 in the OpenAI error shape.
fn assert_refused((status, headers, body): (u16, hyper::HeaderMap, String)) {
  assert_eq!(status, 401, "{body}");
  assert_eq!(headers["www-authenticate"], "Bearer");
  assert_eq!(headers["content-type"], "application/json");
  let error: Value = serde_json::from_str(&body).unwrap();
  assert_eq!(error["error"]["type"], "authentication_error", "{body}");
  assert_eq!(error["error"]["code"], "401", "{body}");
  assert!(error["error"]["message"].as_str().is_some_and(|message| !message.is_empty()), "{body}");
}

#[tokio::test]
async fn a_request_without_the_gateways_key_is_refused_401_at_the_front() {
  let world = World::over(Scratch::blank().await, "").await.start().await;

  // Before any table stands the key is still asked for first.
  assert_refused(read_whole(world.open_bare("GET", "/v1/models", &[], "").await).await);

  Migrator::up(world.store.db(), None).await.unwrap();
  world.write(PAIR).await;
  let table = world.serving(None).await;

  let wrong = format!("Bearer {GATEWAY_KEY}-wrong");
  let basic = format!("Basic {GATEWAY_KEY}");
  let refusals: [&[(&str, &str)]; 5] = [
    &[],
    &[("authorization", &wrong)],
    &[("authorization", "Bearer ")],
    &[("authorization", &basic)],
    &[("x-api-key", GATEWAY_KEY)],
  ];
  for headers in refusals {
    for path in ["/v1/models", "/v1/chat/completions", "/v1/messages", "/health/liveliness", "/held"] {
      assert_refused(read_whole(world.open_bare("POST", path, headers, "{}").await).await);
    }
  }
  // Nothing refused reached the child.
  assert!(world.events("holding").is_empty());

  let keyed = format!("bearer {GATEWAY_KEY}");
  let (status, headers, body) = read_whole(
    world
      .open_bare("POST", "/v1/chat/completions", &[("authorization", &keyed)], "{}")
      .await,
  )
  .await;
  assert_eq!(status, 200, "{body}");
  assert_eq!(headers["x-standin"], table.as_str());
  let asked: Value = serde_json::from_str(&body).unwrap();
  assert_eq!(asked["path"], "/v1/chat/completions");
  assert_eq!(asked["headers"]["authorization"], keyed);
}

#[tokio::test]
async fn a_table_whose_child_does_not_start_leaves_the_one_serving() {
  let world = World::new(PAIR).await.start().await;
  let first = world.serving(None).await;

  std::fs::write(world.root().join("fail-start"), "").unwrap();
  world.write(SINGLE).await;
  until("the new child to fail", || !world.events("failed").is_empty()).await;
  until("the gateway to say so", || world.said().contains("did not start")).await;

  assert_eq!(world.answered_by().await, first);
  let status = world.status().unwrap();
  assert_eq!(status["serving"], first.as_str());
  assert!(world.events("stopped").is_empty());
}

#[tokio::test]
async fn a_child_that_fails_is_answered_502() {
  let world = World::new(PAIR).await.start().await;
  world.serving(None).await;

  let (status, _, _) = world.ask("GET", "/die", &[], "").await;
  assert_eq!(status, 502);
  let (status, _, body) = world.ask("GET", "/v1/models", &[], "").await;
  assert_eq!(status, 502);
  assert!(body.starts_with("the gateway could not reach its table"), "{body}");
}

#[tokio::test]
async fn a_stopped_gateway_stops_its_children_and_takes_its_status_away() {
  let mut world = World::new(PAIR).await.start().await;
  let table = world.serving(None).await;

  world.terminate();
  let gateway = world.gateway.as_mut().unwrap();
  let bound = Instant::now() + Duration::from_secs(20);
  let ended = loop {
    if let Some(ended) = gateway.try_wait().unwrap() {
      break ended;
    }
    assert!(Instant::now() < bound, "the gateway did not end on SIGTERM");
    tokio::time::sleep(Duration::from_millis(20)).await;
  };
  assert_eq!(ended.code(), Some(0));
  assert!(world.events("stopped").iter().any(|entry| entry["table"] == table.as_str()));
  assert!(!world.folder().join("status.json").exists());
  assert!(!world.folder().join(format!("config-{table}.yaml")).exists());
}

// A backend of the test's own: it keeps every chat request it is sent and answers each with one word.
async fn echo_backend() -> (u16, std::sync::Arc<std::sync::Mutex<Vec<Value>>>) {
  let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
  let kept = seen.clone();
  let app = axum::Router::new().route(
    "/v1/chat/completions",
    axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
      let kept = kept.clone();
      async move {
        kept.lock().unwrap().push(body);
        axum::Json(json!({
          "id": "chatcmpl-echo",
          "object": "chat.completion",
          "created": 1,
          "model": "echo-model",
          "choices": [{ "index": 0, "message": { "role": "assistant", "content": "done" }, "finish_reason": "stop" }],
          "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 },
        }))
      }
    }),
  );
  let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
  let port = listener.local_addr().unwrap().port();
  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
  (port, seen)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_runs_long_tools_reach_the_backend_through_litellm_in_the_field_the_engines_read() {
  let (port, seen) = echo_backend().await;
  let echo = format!(
    "INSERT INTO registered_models VALUES ('echo', 'Echo', 'echo-model', 32768, 4096, 0, 0, 3, 0);
     INSERT INTO registered_model_addresses VALUES ('echo', 'local', 1, 'http://127.0.0.1:{port}/v1/chat/completions', NULL, 0, NULL);"
  );
  let world = World::new(&echo).await.on_litellm().start().await;
  world.serving_within(None, Duration::from_secs(180)).await;

  // A turn that called the shell and carries its result back, as Claude Code sends one.
  let turn = json!({
    "model": "echo",
    "max_tokens": 64,
    "tools": [{ "name": "Bash", "description": "Runs a command.", "input_schema": { "type": "object", "properties": { "command": { "type": "string" } } } }],
    "messages": [
      { "role": "user", "content": "List the folder." },
      { "role": "assistant", "content": [{ "type": "tool_use", "id": "toolu_01", "name": "Bash", "input": { "command": "ls" } }] },
      { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "toolu_01", "content": "a.txt" }] },
    ],
  })
  .to_string();
  let key = format!("Bearer {GATEWAY_KEY}");
  let asked = |long_tools: Option<&'static str>| {
    let mut headers = vec![("authorization", key.as_str()), ("content-type", "application/json")];
    headers.extend(long_tools.map(|names| ("x-bmo-long-tools", names)));
    headers
  };

  let (status, _, body) = world
    .ask("POST", "/v1/messages", &asked(Some("Bash,Agent,Task,WebFetch,WebSearch")), &turn)
    .await;
  assert_eq!(status, 200, "{body}\n{}", world.said());
  let (status, _, body) = world.ask("POST", "/v1/messages", &asked(None), &turn).await;
  assert_eq!(status, 200, "{body}");

  let seen = seen.lock().unwrap().clone();
  assert_eq!(seen.len(), 2, "{seen:?}");
  assert_eq!(seen[0]["bmo_long_tools"], "Bash,Agent,Task,WebFetch,WebSearch", "{}", seen[0]);
  let last = seen[0]["messages"].as_array().unwrap().last().unwrap();
  assert_eq!((&last["role"], &last["tool_call_id"]), (&json!("tool"), &json!("toolu_01")), "{last}");
  assert!(seen[1].get("bmo_long_tools").is_none(), "{}", seen[1]);
}
