//! A person's personal apps shared within their company through agency's server (docs/person-nodes.md, 「Shared
//! apps」): a maker's app shared with their team and heads, its source read off their PC's real runner and kept in a
//! stand-in of `bmo-pocket catalogue`; a target outside the maker's ceiling and one of another company refused; a
//! colleague of the same unit seeing and installing it, their PC receiving the source, building it and putting it
//! home standing for the shared app, their own switch turned on; a colleague of another unit and a person of another
//! company never meeting it; a new version reaching the copy as the copies are kept; a head's recall taking the copy
//! away; and a copy dropped and adopted. The account service stands in with the chain's links as they are written.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agency_server::catalogue::{self, Asks, Fault, Reader};
use agency_server::personal_apps;
use agency_server::places::Ground;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{HeaderMap, StatusCode};
use serde_json::{Value, json};

use crate::support::grounds::seeded_with;
use crate::support::scratch::Scratch;

// A stand-in of `bmo-pocket`: the owner verbs as the personal apps' tests keep them, `--app` kept on the pointer, and
// the catalogue verbs agency speaks, each record and bundle kept as the tool host's catalogue keeps them.
const POCKET: &str = r#"#!/usr/bin/env python3
import hashlib, json, os, shutil, sys, time, tomllib
args = sys.argv[1:]
root = args[args.index("--data-root") + 1]
def opt(name, default=None):
    return args[args.index(name) + 1] if name in args else default
def source_hash(files):
    digest = hashlib.sha256()
    for path in sorted(files, key=lambda one: one.encode()):
        data = files[path].encode()
        digest.update(path.encode() + b"\0" + str(len(data)).encode() + b"\0" + data)
    return digest.hexdigest()
if args[2] == "catalogue":
    verb = args[3]
    if verb == "promotions":
        print("[]"); sys.exit(0)
    company = opt("--company")
    kept = os.path.join(root, "tool-host", "catalogue", company)
    os.makedirs(os.path.join(kept, "apps"), exist_ok=True)
    def load(id_):
        file = os.path.join(kept, "apps", id_ + ".json")
        if not os.path.exists(file):
            sys.stderr.write("bmo-pocket: the catalogue of %s holds no app %s\n" % (company, id_)); sys.exit(1)
        return json.load(open(file))
    def save(record):
        json.dump(record, open(os.path.join(kept, "apps", record["id"] + ".json"), "w"))
        print(json.dumps(record))
    positional = [word for at, word in enumerate(args[4:]) if not word.startswith("--") and not args[4 + at - 1].startswith("--")]
    if verb == "list":
        apps = [json.load(open(os.path.join(kept, "apps", file))) for file in sorted(os.listdir(os.path.join(kept, "apps")))]
        seen = os.path.join(kept, "seen", (opt("--reader") or "") + ".json")
        print(json.dumps({"apps": apps, "seenAt": json.load(open(seen))["at"] if os.path.exists(seen) else None}))
    elif verb == "show":
        print(json.dumps(load(positional[0])))
    elif verb == "share":
        bundle = json.load(open(opt("--bundle")))
        manifest = tomllib.loads(bundle["files"]["tool.toml"])
        name = manifest["tool"]["name"]
        maker = opt("--maker")
        id_ = maker + "-" + name
        file = os.path.join(kept, "apps", id_ + ".json")
        record = json.load(open(file)) if os.path.exists(file) else {"id": id_, "key": "shared-%s-%s-%s" % (company, maker, name.replace("_", "-")), "name": name, "title": name, "description": manifest["entry"][0]["description"], "maker": maker, "owner": maker, "unit": opt("--unit"), "targets": [], "versions": [], "withdrawn": None, "installs": {}, "promotion": None, "sharedAt": 1, "updatedAt": 1}
        version = opt("--version")
        os.makedirs(os.path.join(kept, "sources", id_), exist_ok=True)
        json.dump(bundle, open(os.path.join(kept, "sources", id_, version + ".json"), "w"))
        permissions = manifest.get("permissions", {})
        record["targets"] = json.loads(opt("--targets"))
        record["withdrawn"] = None
        record["versions"].append({"version": version, "sourceHash": source_hash(bundle["files"]), "permissions": {"network": permissions.get("network", []), "secrets": permissions.get("secrets", [])}, "sharedAt": int(time.time() * 1000), "review": {"state": "pending"}, "approval": {"required": "--review-required" in args, "by": None}})
        save(record)
    elif verb == "source":
        record = load(positional[0])
        version = opt("--version") or record["versions"][-1]["version"]
        shared = [one for one in record["versions"] if one["version"] == version][0]
        files = json.load(open(os.path.join(kept, "sources", record["id"], version + ".json")))["files"]
        print(json.dumps({"version": version, "sourceHash": shared["sourceHash"], "files": files}))
    elif verb == "install":
        record = load(positional[0])
        record["installs"][opt("--account")] = {"version": opt("--version"), "how": opt("--how"), "at": 1}
        save(record)
    elif verb == "uninstall":
        record = load(positional[0])
        record["installs"].pop(opt("--account"), None)
        save(record)
    elif verb == "withdraw":
        record = load(positional[0]); record["withdrawn"] = 5; save(record)
    elif verb == "review":
        record = load(positional[0])
        for one in record["versions"]:
            if one["version"] == opt("--version"):
                one["review"] = {"state": opt("--state"), "report": open(opt("--report")).read(), "at": 1}
        save(record)
    elif verb == "seen":
        os.makedirs(os.path.join(kept, "seen"), exist_ok=True)
        json.dump({"at": 7}, open(os.path.join(kept, "seen", opt("--account") + ".json"), "w"))
        print(json.dumps({"at": 7}))
    else:
        sys.stderr.write("bmo-pocket: no verb %s here\n" % verb); sys.exit(2)
    sys.exit(0)
verb = args[2]
owner = opt("--owner")
owned = os.path.join(root, "tool-host", "owners", owner)
pointers = os.path.join(owned, "registry")
if verb == "put":
    version = opt("--version")
    folder = args[-1]
    manifest = tomllib.loads(open(os.path.join(folder, "tool.toml")).read())
    name = manifest["tool"]["name"]
    placed = os.path.join(owned, "tools", name, version)
    if not os.path.isdir(placed):
        shutil.copytree(folder, placed)
    os.makedirs(pointers, exist_ok=True)
    pointer = {"name": name, "version": version, "dir": placed, "origin": "machine:" + owner, "owner": owner, "putAt": 5}
    if opt("--app"):
        pointer["app"] = opt("--app")
    json.dump(pointer, open(os.path.join(pointers, name + ".json"), "w"))
    print("put %s's %s %s" % (owner, name, version))
elif verb == "list":
    rows = []
    for file in sorted(os.listdir(pointers)) if os.path.isdir(pointers) else []:
        pointer = json.load(open(os.path.join(pointers, file)))
        manifest = tomllib.loads(open(os.path.join(pointer["dir"], "tool.toml")).read())
        entries = [{"name": "my/" + entry["name"], "description": entry["description"]} for entry in manifest.get("entry", [])]
        rows.append(dict(pointer, entries=entries, permissions={"network": [], "secrets": []}))
    print(json.dumps(rows))
elif verb == "take":
    name = args[-1]
    pointer = os.path.join(pointers, name + ".json")
    if not os.path.exists(pointer):
        sys.stderr.write("bmo-pocket: %s holds no personal tool %s\n" % (owner, name)); sys.exit(1)
    os.remove(pointer)
    print("took %s's %s" % (owner, name))
"#;

const ADMINISTRATOR: &str = "1";
const MAKER: i64 = 17;
const TEAMMATE: i64 = 21;
const NEIGHBOUR: i64 = 31;
const STRANGER: i64 = 51;
const HEAD: i64 = 9;

// The account service standing in: people's paths, the head of unit 40, and the chain's links as they are written.
#[derive(Clone, Default)]
struct Chain {
  links: Arc<Mutex<BTreeMap<(String, String), Value>>>,
  locks: Arc<Mutex<BTreeMap<(String, String), String>>>,
  rules: Arc<Mutex<BTreeMap<String, Value>>>,
}

fn units_of(person: i64) -> Vec<i64> {
  match person {
    MAKER | TEAMMATE | HEAD => vec![3, 40],
    NEIGHBOUR => vec![3, 41],
    _ => vec![5, 50],
  }
}

fn path_of(units: &[i64], person: Option<i64>) -> Vec<String> {
  std::iter::once("platform".to_owned())
    .chain(units.iter().map(|unit| format!("unit:{unit}")))
    .chain(person.map(|person| format!("person:{person}")))
    .collect()
}

impl Chain {
  fn said(&self, key: &str, link: &str) -> Option<Value> {
    self.links.lock().unwrap().get(&(key.to_owned(), link.to_owned())).cloned()
  }

  fn locked_above(&self, key: &str, path: &[String]) -> bool {
    let locks = self.locks.lock().unwrap();

    path
      .iter()
      .any(|link| locks.get(&(key.to_owned(), link.clone())).is_some_and(|one| one == "lock"))
  }

  // A kind resolved down a path: an offered flag narrows, every other kind takes its nearest word.
  fn resolve(&self, key: &str, path: &[String]) -> Value {
    if key == "personal-apps.share-scope" {
      return json!({ "value": ["team", "heads"], "path": path });
    }

    if key.ends_with(".offered") {
      let company = path.get(1).cloned().unwrap_or_default();
      let offered = self.rules.lock().unwrap().get(key).and_then(Value::as_array).is_some_and(|rules| {
        rules.iter().any(|rule| rule["who"] == json!(company) && rule["value"] == true)
      });
      let off = path.iter().skip(1).any(|link| self.said(key, link) == Some(json!(false)));

      return json!({ "value": offered && !off, "path": path });
    }

    let nearest = path.iter().rev().find_map(|link| {
      self.said(key, link).filter(|value| !value.is_null()).map(|value| (link.clone(), value))
    });

    match nearest {
      Some((from, value)) => json!({ "value": value, "from": from, "path": path }),
      None => json!({ "value": null, "path": path }),
    }
  }

  // Whether the caller writes the link: the head of unit 40 its link, and a person their own.
  fn writes(&self, caller: i64, link: &str) -> bool {
    link == format!("person:{caller}") || (caller == HEAD && link == "unit:40")
  }
}

fn caller(headers: &HeaderMap) -> i64 {
  headers.get("owner-key").and_then(|key| key.to_str().ok()?.parse().ok()).unwrap_or(0)
}

async fn account_service(chain: Chain) -> String {
  use axum::routing::{get, put};

  let app = axum::Router::new()
    .route(
      "/chain/kinds/{key}/resolve",
      get(|State(chain): State<Chain>, UrlPath(key): UrlPath<String>, Query(query): Query<BTreeMap<String, String>>| async move {
        let path = match (query.get("person"), query.get("unit")) {
          (Some(person), _) => {
            let person: i64 = person.parse().unwrap();

            path_of(&units_of(person), Some(person))
          }
          (_, Some(unit)) => {
            let unit: i64 = unit.parse().unwrap();

            path_of(&if unit == 41 { vec![3, 41] } else if unit / 10 == 5 { vec![5, unit] } else { vec![3, unit] }, None)
          }
          _ => Vec::new(),
        };

        axum::Json(chain.resolve(&key, &path))
      }),
    )
    .route(
      "/chain/kinds/{key}",
      put(|| async { axum::Json(json!({})) }),
    )
    .route(
      "/chain/kinds/{key}/links/{link}",
      get(|State(chain): State<Chain>, UrlPath((key, link)): UrlPath<(String, String)>, headers: HeaderMap| async move {
        let caller = caller(&headers);

        if !chain.writes(caller, &link) {
          return (StatusCode::FORBIDDEN, axum::Json(json!({ "code": "forbidden" })));
        }

        let above = path_of(&units_of(caller), None);
        let writable = !link.starts_with("person:") || !chain.locked_above(&key, &above);

        (StatusCode::OK, axum::Json(json!({ "link": link, "writable": writable })))
      }),
    )
    .route(
      "/chain/kinds/{key}/links/{link}/value",
      put(|State(chain): State<Chain>, UrlPath((key, link)): UrlPath<(String, String)>, headers: HeaderMap, axum::Json(body): axum::Json<Value>| async move {
        let caller = caller(&headers);

        if !chain.writes(caller, &link) {
          return (StatusCode::FORBIDDEN, axum::Json(json!({ "code": "forbidden" })));
        }

        if link.starts_with("person:") && chain.locked_above(&key, &path_of(&units_of(caller), None)) {
          return (StatusCode::CONFLICT, axum::Json(json!({ "code": "locked" })));
        }

        chain.links.lock().unwrap().insert((key, link), body["value"].clone());
        (StatusCode::OK, axum::Json(json!({})))
      }),
    )
    .route(
      "/chain/kinds/{key}/links/{link}/delegation",
      put(|State(chain): State<Chain>, UrlPath((key, link)): UrlPath<(String, String)>, headers: HeaderMap, axum::Json(body): axum::Json<Value>| async move {
        if !chain.writes(caller(&headers), &link) {
          return (StatusCode::FORBIDDEN, axum::Json(json!({ "code": "forbidden" })));
        }

        chain.locks.lock().unwrap().insert((key, link), body["delegation"].as_str().unwrap_or_default().to_owned());
        (StatusCode::OK, axum::Json(json!({})))
      }),
    )
    .route(
      "/chain/kinds/{key}/links/platform/rules",
      put(|State(chain): State<Chain>, UrlPath(key): UrlPath<String>, headers: HeaderMap, axum::Json(body): axum::Json<Value>| async move {
        if caller(&headers).to_string() != ADMINISTRATOR {
          return (StatusCode::FORBIDDEN, axum::Json(json!({ "code": "forbidden" })));
        }

        chain.rules.lock().unwrap().insert(key, body["rules"].clone());
        (StatusCode::OK, axum::Json(json!({})))
      }),
    )
    .route(
      "/units/{unit}",
      get(|UrlPath(unit): UrlPath<i64>| async move { axum::Json(json!({ "id": unit, "heads": [HEAD], "left": [], "path": [] })) }),
    )
    .route(
      "/units/{unit}/subtree",
      get(|| async {
        axum::Json(json!({ "units": [{ "id": 40, "heads": [{ "user": HEAD }], "members": [{ "user": MAKER }, { "user": TEAMMATE }] }] }))
      }),
    )
    .with_state(chain);
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
  address
}

struct Stood {
  _scratch: Scratch,
  home: tempfile::TempDir,
  ground: Arc<Ground>,
  chain: Chain,
}

impl Stood {
  // A person's PC: its own data root, holding 'my apps' and its runner's root, its runner answering on the link's
  // runner socket by the real runner of this build.
  async fn pc(&self, person: i64) -> PathBuf {
    let node = format!("u{person}pc");
    let data = self.home.path().join(format!("pc-{person}"));
    let root = data.join(node_link::runner::PERSONAL_TOOLS);
    let socket = self.ground.roots.runner_socket(&node);
    let spec_path = self.ground.roots.spec_path(&node);

    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(spec_path.parent().unwrap()).unwrap();
    sea_orm::ConnectionTrait::execute_unprepared(
      &self.ground.db,
      &format!(
        r#"INSERT INTO "person_nodes" ("owner_account", "node", "shared", "set_at") VALUES ('{person}', '{node}', false, 1)"#
      ),
    )
    .await
    .unwrap();
    std::fs::write(
      &spec_path,
      json!({
        "node": node, "ssh": [], "root": "/pc", "binary": "/pc/deploy/bin/node-link",
        "herdr": { "local": "/h.sock", "remote": "/rh.sock" }, "bridge": "/b.sock", "deploy": "/d",
        "control": "/c.sock", "proc": "/proc-here", "state": "/state.json",
        "runner": socket, "runner_owner": person.to_string(),
      })
      .to_string(),
    )
    .unwrap();
    serve_runner(&socket, &root, &person.to_string());
    data
  }
}

// The real runner of this build, started for each connection on the socket, the connection its passage.
fn serve_runner(socket: &Path, root: &Path, owner: &str) {
  let listener = std::os::unix::net::UnixListener::bind(socket).unwrap();
  let (root, owner) = (root.to_path_buf(), owner.to_owned());

  std::thread::spawn(move || {
    for stream in listener.incoming().flatten() {
      let input = stream.try_clone().unwrap();
      let mut child = std::process::Command::new(crate::support::binary::path("node-link"))
        .args(["runner", "--root"])
        .arg(&root)
        .args(["--owner", &owner])
        .stdin(std::process::Stdio::from(std::os::fd::OwnedFd::from(input)))
        .stdout(std::process::Stdio::from(std::os::fd::OwnedFd::from(stream)))
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();

      std::thread::spawn(move || {
        let _ = child.wait();
      });
    }
  });
}

// The maker's app in their PC's 'my apps', what it does `said`.
fn app(pc: &Path, name: &str, said: &str) {
  let dir = pc.join(node_link::runner::MY_APPS).join(name);

  std::fs::create_dir_all(&dir).unwrap();
  std::fs::write(
    dir.join("tool.toml"),
    format!(
      "[tool]\nname = \"{name}\"\nprotocol = 1\nrun = [\"python3\", \"main.py\"]\n\n[[entry]]\nnamespace = \"my\"\nname = \"{name}\"\ndescription = \"{said}\"\ninput = \"input.json\"\ncapabilities = [\"my.own\"]\n"
    ),
  )
  .unwrap();
  std::fs::write(dir.join("input.json"), r#"{ "type": "object" }"#).unwrap();
  std::fs::write(dir.join("main.py"), "print('{}')").unwrap();
}

async fn stood() -> Stood {
  let chain = Chain::default();
  let address = account_service(chain.clone()).await;
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let pocket = home.path().join("fake-pocket");

  std::fs::write(&pocket, POCKET).unwrap();
  std::fs::set_permissions(&pocket, std::fs::Permissions::from_mode(0o755)).unwrap();

  let variables = BTreeMap::from([
    (personal_apps::POCKET_VARIABLE.to_owned(), pocket.to_string_lossy().into_owned()),
    ("WEB_ADDRESS".to_owned(), address),
    ("WEB_TOKEN".to_owned(), "chain-token".to_owned()),
  ]);
  let ground = seeded_with(scratch.db(), home.path(), &variables).await;

  scratch
    .run(
      &format!(
        r#"INSERT INTO "settings" ("key", "value") VALUES ('users_administrator', '{ADMINISTRATOR}')"#
      ),
      vec![],
    )
    .await;

  Stood {
    _scratch: scratch,
    home,
    ground: Arc::new(ground),
    chain,
  }
}

async fn reader(stood: &Stood, person: i64) -> Reader {
  catalogue::reader(&stood.ground, &person.to_string(), None).await.unwrap()
}

fn home_pointer(stood: &Stood, account: i64, name: &str) -> Option<Value> {
  let file = stood.ground.roots.data.join(format!("tool-host/owners/{account}/registry/{name}.json"));

  serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()
}

#[test]
fn a_review_is_handed_what_the_manifest_asks_and_every_file_under_its_path() {
  let input = catalogue::review_input(
    &Asks {
      network: vec!["rates.example.org".to_owned()],
      secrets: Vec::new(),
    },
    &json!({ "main.py": "print('{}')", "tool.toml": "[tool]" }),
  );

  assert!(input.contains("the network at: rates.example.org."));
  assert!(input.contains("the secrets: none."));
  assert!(input.contains("=== main.py ===\nprint('{}')"));
  assert!(input.find("=== main.py ===") < input.find("=== tool.toml ==="));
  assert!(
    Asks {
      network: vec!["a".to_owned()],
      secrets: Vec::new()
    }
    .grows_over(&Asks::default())
  );
  assert!(!Asks::default().grows_over(&Asks {
    network: vec!["a".to_owned()],
    secrets: Vec::new()
  }));
}

#[tokio::test]
async fn an_app_is_shared_seen_installed_updated_and_recalled_within_the_company_alone() {
  let stood = stood().await;
  let ground = &stood.ground;
  let maker_pc = stood.pc(MAKER).await;
  let teammate_pc = stood.pc(TEAMMATE).await;

  app(&maker_pc, "weekly", "Sums up the week.");

  // The maker's app is placed and pointed at on their PC first, as their BMO's put does.
  let put = personal_apps::put(ground, &MAKER.to_string(), "weekly").await.unwrap();
  let first = put.version.clone().unwrap();
  let maker = reader(&stood, MAKER).await;

  // A target beyond the maker's ceiling, and one of another company, are refused.
  assert!(matches!(
    catalogue::share(ground, &maker, "weekly", &[json!({ "company": true })]).await,
    Err(Fault::ScopeWithheld(scope)) if scope == "company"
  ));
  assert!(matches!(
    catalogue::share(ground, &maker, "weekly", &[json!({ "person": STRANGER })]).await,
    Err(Fault::OtherCompany(_))
  ));

  let shared = catalogue::share(ground, &maker, "weekly", &[]).await.unwrap();

  // It is offered to the maker's company alone, by the platform's rule naming it.
  assert_eq!(
    stood.chain.rules.lock().unwrap().get("app.shared-3-17-weekly.offered"),
    Some(&json!([{ "who": "unit:3", "value": true }]))
  );
  assert_eq!(shared.id, "17-weekly");
  assert_eq!(shared.version, first);
  assert_eq!(shared.targets, vec![json!({ "team": true }), json!({ "heads": true })]);
  assert!(!shared.may.install, "a maker installs no copy of their own app");

  // A teammate sees it and installs it; a colleague of another unit and a stranger never meet it.
  let teammate = reader(&stood, TEAMMATE).await;
  let listed = catalogue::list(ground, &teammate).await.unwrap();

  assert_eq!(listed.apps.len(), 1);
  assert!(listed.apps[0].may.install);
  assert_eq!(listed.notices[0].kind, "shared");
  assert!(catalogue::list(ground, &reader(&stood, NEIGHBOUR).await).await.unwrap().apps.is_empty());
  assert!(catalogue::list(ground, &reader(&stood, STRANGER).await).await.unwrap().apps.is_empty());
  assert!(matches!(
    catalogue::install(ground, &reader(&stood, NEIGHBOUR).await, "17-weekly").await,
    Err(Fault::NotFound(_))
  ));

  let installed = catalogue::install(ground, &teammate, "17-weekly").await.unwrap();

  assert_eq!(installed.installed.as_ref().unwrap().version, first);
  assert_eq!(installed.installed.as_ref().unwrap().how, "self");
  assert!(
    teammate_pc.join("my-apps/weekly/main.py").exists(),
    "the source reaches the teammate's PC"
  );
  assert_eq!(home_pointer(&stood, TEAMMATE, "weekly").unwrap()["app"], "shared-3-17-weekly");
  assert_eq!(stood.chain.said("app.shared-3-17-weekly.on", "person:21"), Some(json!(true)));

  // A new version asking for nothing more reaches the copy as the copies are kept.
  tokio::time::sleep(std::time::Duration::from_millis(3)).await;
  app(&maker_pc, "weekly", "Sums up the week, shorter.");

  let second = personal_apps::put(ground, &MAKER.to_string(), "weekly").await.unwrap().version.unwrap();

  catalogue::share(ground, &maker, "weekly", &[]).await.unwrap();
  catalogue::keep(ground, &TEAMMATE.to_string()).await.unwrap();

  let moved = catalogue::entry(ground, &teammate, "17-weekly").await.unwrap();

  assert_eq!(moved.installed.unwrap().version, second);

  // The head of the unit recalls it there: the copy is taken away as the teammate's copies are kept.
  let head = reader(&stood, HEAD).await;
  let headed = catalogue::entry(ground, &head, "17-weekly").await.unwrap();

  assert_eq!(headed.may.recall, vec!["unit:40".to_owned()]);
  assert!(matches!(
    catalogue::recall(ground, &teammate, "17-weekly", Some(40)).await,
    Err(Fault::Forbidden(_))
  ));
  catalogue::recall(ground, &head, "17-weekly", Some(40)).await.unwrap();
  assert_eq!(stood.chain.said("app.shared-3-17-weekly.offered", "unit:40"), Some(json!(false)));
  catalogue::keep(ground, &TEAMMATE.to_string()).await.unwrap();
  assert!(home_pointer(&stood, TEAMMATE, "weekly").is_none(), "the recalled copy is taken away");
  assert!(catalogue::entry(ground, &maker, "17-weekly").await.unwrap().installs == 0);
}

#[tokio::test]
async fn a_head_turns_an_app_on_for_their_unit_and_a_copy_is_dropped_or_adopted() {
  let stood = stood().await;
  let ground = &stood.ground;
  let maker_pc = stood.pc(MAKER).await;
  let teammate_pc = stood.pc(TEAMMATE).await;

  app(&maker_pc, "tally", "Counts the days.");
  personal_apps::put(ground, &MAKER.to_string(), "tally").await.unwrap();

  let maker = reader(&stood, MAKER).await;

  catalogue::share(ground, &maker, "tally", &[]).await.unwrap();

  // The head turns it on for unit 40, locked: the teammate's copy is installed as their copies are kept, and they may
  // neither drop it nor adopt it.
  let head = reader(&stood, HEAD).await;

  catalogue::switch_unit(ground, &head, "17-tally", 40, true, true).await.unwrap();
  catalogue::keep(ground, &TEAMMATE.to_string()).await.unwrap();

  let teammate = reader(&stood, TEAMMATE).await;
  let copy = catalogue::entry(ground, &teammate, "17-tally").await.unwrap();

  assert_eq!(copy.installed.as_ref().unwrap().how, "unit");
  assert!(matches!(
    catalogue::uninstall(ground, &teammate, "17-tally").await,
    Err(Fault::Locked(_))
  ));

  // Unlocked, the teammate adopts the copy as their own: it no longer stands for the shared app.
  catalogue::switch_unit(ground, &head, "17-tally", 40, true, false).await.unwrap();

  let adopted = catalogue::adopt(ground, &teammate, "17-tally").await.unwrap();

  assert!(adopted.installed.is_none());
  assert!(home_pointer(&stood, TEAMMATE, "tally").unwrap().get("app").is_none());
  assert!(!teammate_pc.join("my-apps/tally").join(node_link::keeping::RECEIVED_MARK).exists());
  assert_eq!(stood.chain.said("app.shared-3-17-tally.on", "person:21"), Some(json!(false)));

  // Kept again, the adopted app is the teammate's own and nothing is installed over it.
  catalogue::keep(ground, &TEAMMATE.to_string()).await.unwrap();
  assert!(catalogue::entry(ground, &teammate, "17-tally").await.unwrap().installed.is_none());

  // A withdrawn app is installed by nobody new.
  assert!(matches!(
    catalogue::withdraw(ground, &teammate, "17-tally").await,
    Err(Fault::Forbidden(_))
  ));
  catalogue::withdraw(ground, &maker, "17-tally").await.unwrap();
  assert!(matches!(
    catalogue::install(ground, &teammate, "17-tally").await,
    Err(Fault::Withdrawn(_))
  ));
}

// "the administrator is a member of their own company's catalogue: they install a copy there on the runner their
// records name, as any member does, and keep recalling at the platform; in another company they install nothing"
#[tokio::test]
async fn the_administrator_installs_in_their_own_company_alone() {
  let stood = stood().await;
  let ground = &stood.ground;
  let own_pc = stood.pc(STRANGER).await;
  let other_pc = stood.pc(MAKER).await;
  // The administrator's runner is this deployment's node link, standing in the deploy tree.
  let bin = ground.roots.deploy.join("bin");

  std::fs::create_dir_all(&bin).unwrap();
  std::os::unix::fs::symlink(crate::support::binary::path("node-link"), bin.join("node-link")).unwrap();

  // An app of the administrator's own company, and one of another.
  app(&own_pc, "daily", "Says the day.");
  personal_apps::put(ground, &STRANGER.to_string(), "daily").await.unwrap();
  catalogue::share(ground, &reader(&stood, STRANGER).await, "daily", &[]).await.unwrap();
  app(&other_pc, "weekly", "Sums up the week.");
  personal_apps::put(ground, &MAKER.to_string(), "weekly").await.unwrap();
  catalogue::share(ground, &reader(&stood, MAKER).await, "weekly", &[]).await.unwrap();

  let own = catalogue::reader(ground, ADMINISTRATOR, Some(5)).await.unwrap();
  let other = catalogue::reader(ground, ADMINISTRATOR, Some(3)).await.unwrap();

  assert!(own.member().is_some(), "their home unit stands in company 5");
  assert!(other.member().is_none());

  // In another company: every app seen and recalled at the platform, none installed.
  let weekly = catalogue::entry(ground, &other, "17-weekly").await.unwrap();

  assert!(!weekly.may.install);
  assert_eq!(weekly.may.recall, vec!["platform".to_owned()]);
  assert!(matches!(
    catalogue::install(ground, &other, "17-weekly").await,
    Err(Fault::Forbidden(_))
  ));

  // In their own company: installed as a member, received into their own 'my apps' and put home for them.
  let daily = catalogue::entry(ground, &own, "51-daily").await.unwrap();

  assert!(daily.may.install);
  assert_eq!(daily.may.recall, vec!["platform".to_owned()]);

  let installed = catalogue::install(ground, &own, "51-daily").await.unwrap();

  assert_eq!(installed.installed.as_ref().unwrap().how, "self");
  assert!(installed.may.uninstall && installed.may.clone);
  assert!(
    ground.roots.data.join("my-apps/daily/main.py").exists(),
    "the source reaches the administrator's runner"
  );
  assert_eq!(home_pointer(&stood, 1, "daily").unwrap()["app"], "shared-5-51-daily");
}
