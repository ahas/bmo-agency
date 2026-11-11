//! A ground of a test's own on a scratch store, the store seeded as the deployment seeds it, and DeepSeek's
//! road standing on it so a run may go out on a provider of the machine's own: its key and its gateway member.

use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::order_run::dispatch;
use agency_server::places::Ground;
use agency_server::seeds::deployment;
use cache_siren::{Balance, Slot};
use harness_providers::Machine;
use sea_orm::DatabaseConnection;

/// A ground on a store, under a home of the test's own with no program on its path, marked a test's by that
/// home (docs/live-claude-guard.md, 「Second layer: the guard in agency's code」).
pub fn ground_on(db: DatabaseConnection, home: &std::path::Path) -> Ground {
  let variables: HashMap<String, String> = HashMap::new();
  let service = Service::new(db, Config::read(|name| variables.get(name).cloned()).unwrap());
  let home_path = home.to_string_lossy().into_owned();
  let env = vec![
    ("HOME".to_owned(), home_path.clone()),
    ("BMO_TEST_HOME".to_owned(), home_path),
    ("PATH".to_owned(), "/nowhere".to_owned()),
  ];

  Ground::new(&service, &Machine::with_env(env, home.join("data")), None)
}

/// DeepSeek's road standing under the home: its key and its flash model as a member of the gateway.
pub fn deepseek_road(home: &std::path::Path) {
  let data = home.join("data");
  let env = data.join("env");

  std::fs::create_dir_all(&env).unwrap();
  for (name, key) in [("deepseek.key", "a key\n"), ("llm-gateway.key", "a key\n")] {
    let file = env.join(name);

    std::fs::write(&file, key).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
  }

  let members = data.join("llm-gateway");

  std::fs::create_dir_all(&members).unwrap();
  std::fs::write(
    members.join("members.json"),
    r#"[{"name":"deepseek-flash","road":"deepseek","provider":"deepseek","model":"deepseek-flash","api_base":"https://api.deepseek.com/v1","key_file":"deepseek.key","context_tokens":1048576,"answer_tokens":32768,"thinking":true}]"#,
  )
  .unwrap();
}

/// A ground seeded as the deployment seeds its store, DeepSeek's road standing on it.
pub async fn seeded(db: &DatabaseConnection, home: &std::path::Path) -> Ground {
  let ground = ground_on(db.clone(), home);

  store::seed::settle(db, &deployment(&ground.machine())).await.unwrap();
  deepseek_road(home);

  ground
}

/// DeepSeek's balance as its reader last kept it.
pub async fn deepseek_balance(ground: &Ground, amount: f64) {
  ground
    .windows
    .replace_balance(
      Slot {
        mode: "deepseek",
        account: "",
      },
      &Balance {
        amount,
        currency: "USD".to_owned(),
        requests: None,
      },
      agency_server::now_ms() / 1000,
    )
    .await
    .unwrap();
}

/// One provider of an order a test lays down: its name, and whether the owner has it on.
#[derive(Clone, Debug)]
pub struct OrderEntry {
  pub name: String,
  pub enabled: bool,
}

/// The providers a test lays down, in its order: each provider's switch thrown as it says, and every role's and
/// supervisor's order put in that order, each keeping its own entry for every provider it names.
pub async fn set_provider_order(db: &DatabaseConnection, order: &[OrderEntry]) -> Result<(), String> {
  for entry in order {
    store::run(
      db,
      r#"INSERT INTO "settings" ("key", "value") VALUES ($1, $2)
         ON CONFLICT ("key") DO UPDATE SET "value" = EXCLUDED."value""#,
      vec![
        format!("provider_switch:{}", entry.name).into(),
        (if entry.enabled { "on" } else { "off" }).into(),
      ],
    )
    .await
    .map_err(|why| why.detail)?;
  }

  for table in ["roles", "supervisors"] {
    let rows = store::all(db, &format!(r#"SELECT "name", "provider_order" FROM "{table}""#), vec![])
      .await
      .map_err(|why| why.detail)?;

    for row in rows {
      let name: String = row.try_get("", "name").map_err(|why| why.to_string())?;
      let held: String = row.try_get("", "provider_order").map_err(|why| why.to_string())?;
      let mut entries: Vec<serde_json::Value> = serde_json::from_str(&held).unwrap_or_default();
      let place = |entry: &serde_json::Value| {
        order
          .iter()
          .position(|one| entry["provider"] == one.name.as_str())
          .unwrap_or(order.len())
      };

      entries.sort_by_key(place);
      store::run(
        db,
        &format!(r#"UPDATE "{table}" SET "provider_order" = $1 WHERE "name" = $2"#),
        vec![serde_json::Value::Array(entries).to_string().into(), name.into()],
      )
      .await
      .map_err(|why| why.detail)?;
    }
  }

  Ok(())
}

/// A slot of the worker role asked for a job's run, as its launch asks it: under the job's owner, its request
/// key and its checkout's path. Let in and bound to the job's run where `let_in`, and left waiting otherwise.
pub async fn job_slot(ground: &Ground, job: i64, let_in: bool) -> dispatch::Request {
  let asked = dispatch::Asked {
    request_key: format!("job-{job}-1"),
    owner: format!("{}{job}", dispatch::JOB_OWNER),
    wanted: dispatch::Wanted {
      role: "worker",
      path: Some("/checkouts/worker"),
      fixed: None,
      agent: None,
    },
    ..dispatch::Asked::default()
  };
  let held = dispatch::enqueue(&ground.db, &ground.events, &asked, dispatch::JOB, async |_, _| Ok(()))
    .await
    .unwrap();

  if !let_in {
    assert!(dispatch::acquire(ground, &held).await.unwrap().is_none(), "the job's slot waits");

    return dispatch::request(&ground.db, &held.token).await.unwrap().unwrap();
  }

  let admitted = dispatch::acquire(ground, &held).await.unwrap().expect("the job's slot is let in");

  dispatch::bind(&ground.db, &ground.events, &admitted, &format!("job-{job}"), None)
    .await
    .unwrap()
}

/// A slot request as the line holds it now: its state, who gave it back and why.
pub async fn slot_now(ground: &Ground, token: &str) -> dispatch::Request {
  dispatch::request(&ground.db, token).await.unwrap().unwrap()
}
