//! The two axes' migration carries a store's live rows over as the owner settled it (docs/agents.md, 「Two
//! axes」): chat rows become supervisors, worker rows repositories, runners and co-worker definitions roles,
//! each row's order built from the provider its route named and the owner's order at its old tier, and the
//! agent rows, tiers, work keys' roads, fold ratios and pool capacities go.

use sea_orm_migration::MigratorTrait;
use serde_json::{Value, json};
use store::migration::{Migrator, TWO_AXES};

use crate::support::scratch::Scratch;

// A blank store brought to the shape before the two axes.
async fn before() -> Scratch {
  let scratch = Scratch::blank().await;
  let steps = Migrator::migrations()
    .iter()
    .take_while(|migration| migration.name() != TWO_AXES)
    .count();

  Migrator::up(scratch.db(), Some(steps as u32)).await.unwrap();

  scratch
}

async fn agent(scratch: &Scratch, columns: &str, values: Vec<sea_orm::Value>) {
  let marks: Vec<String> = (1..=values.len()).map(|at| format!("${at}")).collect();

  scratch
    .run(&format!(r#"INSERT INTO "agents" ({columns}) VALUES ({})"#, marks.join(", ")), values)
    .await;
}

// The live store's shape, in small: the owner's order with two providers off, every tier mapped on each
// provider, the work keys' tiers and routes, the fold ratios, the chat, gate, runner and worker rows, the
// seeded roles and the co-worker definitions.
async fn live(scratch: &Scratch) {
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ('provider_order', $1)"#,
      vec![
        json!([
          {"name": "deepseek", "enabled": true},
          {"name": "registered", "enabled": true},
          {"name": "claude-code", "enabled": true},
          {"name": "codex", "enabled": false},
          {"name": "openrouter", "enabled": false}
        ])
        .to_string()
        .into(),
      ],
    )
    .await;

  let models = [
    ("claude-code", "claude-opus-5-5", "claude-sonnet-5", "claude-haiku-4-5-20251001"),
    ("codex", "gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"),
    ("openrouter", "nemotron", "gemma", "gemma"),
    ("deepseek", "deepseek-flash", "deepseek-flash", "deepseek-flash"),
    ("registered", "flash-next", "flash-next-coder", "flash-next-coder"),
  ];

  for (provider, heavy, middle, light) in models {
    for (tier, model, effort) in [
      ("Supervisor", heavy, "medium"),
      ("Frontier", heavy, "high"),
      ("Reliable", middle, "high"),
      ("Fine", middle, "low"),
      ("Light", light, "low"),
      ("Simple", light, "low"),
    ] {
      let effort = if provider == "registered" { "thinking" } else { effort };
      let ratio = match provider {
        "deepseek" | "openrouter" => 0.5,
        "registered" => 0.95,
        _ => 0.85,
      };

      scratch
        .run(
          r#"INSERT INTO "model_tier_mappings" ("provider", "tier", "model", "effort") VALUES ($1, $2, $3, $4)"#,
          vec![provider.into(), tier.into(), model.into(), effort.into()],
        )
        .await;
      scratch
        .run(
          r#"INSERT INTO "fold_ratios" ("provider", "tier", "ratio", "set_at") VALUES ($1, $2, $3, 0)"#,
          vec![provider.into(), tier.into(), ratio.into()],
        )
        .await;
    }
  }

  for (key, tier) in [
    ("bmo", "Supervisor"),
    ("job-worker", "Frontier"),
    ("search", "Frontier"),
    ("browse", "Frontier"),
    ("translate", "Simple"),
    ("role:builder", "Frontier"),
    ("role:verifier", "Reliable"),
    ("role:digest", "Fine"),
    ("role:scout", "Light"),
    ("role:screener", "Fine"),
    ("role:helper", "Reliable"),
    ("role:transliterator", "Light"),
  ] {
    scratch
      .run(
        r#"INSERT INTO "model_tier_assignments" ("key", "tier") VALUES ($1, $2)"#,
        vec![key.into(), tier.into()],
      )
      .await;
  }

  for (key, mode, registration) in [
    ("bmo", "deepseek", None),
    ("job-worker", "deepseek", None),
    ("translate", "registered", Some("hy-mt")),
  ] {
    scratch
      .run(
        r#"INSERT INTO "fallback_routes" ("key", "route_mode", "registration", "set_at") VALUES ($1, $2, $3, 0)"#,
        vec![key.into(), mode.into(), registration.into()],
      )
      .await;
  }

  agent(
    scratch,
    r#""name", "kind", "role", "label", "session_scope", "turn_shape", "default_answerer", "effort", "builtin""#,
    vec![
      "bmo".into(),
      "chat".into(),
      "chat".into(),
      "Chat agent".into(),
      "agent".into(),
      "window".into(),
      1i64.into(),
      "max".into(),
      1i64.into(),
    ],
  )
  .await;
  agent(
    scratch,
    r#""name", "kind", "role", "session_scope", "turn_shape", "pinned_provider", "pinned_model", "item_worker",
       "brain_folder", "keepalive_stopped""#,
    vec![
      "companion".into(),
      "chat".into(),
      "chat".into(),
      "room".into(),
      "pieces".into(),
      "registered".into(),
      "flash-next".into(),
      "companion-repo".into(),
      "agents/companion".into(),
      1i64.into(),
    ],
  )
  .await;

  for (name, kind, role) in [
    ("output-format", "gate", "gate"),
    ("search", "runner", "searcher"),
    ("browse", "runner", "searcher"),
    ("translator", "runner", "translator"),
  ] {
    agent(scratch, r#""name", "kind", "role""#, vec![name.into(), kind.into(), role.into()]).await;
  }

  // Worker rows: one pinned to DeepSeek's flash model as most were, one on a registered model, one with a
  // support checkout beside it here and one on a node, the video producer and the helper.
  for (name, role, pin, support_of, node, cwd) in [
    ("alpha", "worker", Some(("deepseek", "deepseek-flash")), None, None, "/repos/alpha"),
    (
      "companion-repo",
      "worker",
      Some(("registered", "flash-next")),
      None,
      None,
      "/repos/companion",
    ),
    ("web", "worker", Some(("deepseek", "deepseek-flash")), None, None, "/repos/web"),
    ("web-w2", "worker", None, Some("web"), None, "/repos/web-w2"),
    ("web-n3", "worker", None, Some("web"), Some("n3"), "/remote/web"),
    ("video", "video", None, None, None, "/repos/video"),
    ("helper", "helper", Some(("claude-code", "claude-fable")), None, None, "/repos"),
  ] {
    agent(
      scratch,
      r#""name", "kind", "role", "label", "cwd", "pinned_provider", "pinned_model", "support_of", "node",
         "preamble", "dev_command", "downgrade_heal""#,
      vec![
        name.into(),
        "worker".into(),
        role.into(),
        name.to_uppercase().into(),
        cwd.into(),
        pin.map(|(provider, _)| provider).into(),
        pin.map(|(_, model)| model).into(),
        support_of.into(),
        node.into(),
        format!("{name} preamble").into(),
        "bun dev".into(),
        i64::from(name != "alpha").into(),
      ],
    )
    .await;
  }

  for (place, (name, sessions, cap)) in [
    ("chat", "keep", Some(8i64)),
    ("worker", "keep", Some(8)),
    ("searcher", "fresh", Some(4)),
    ("translator", "fresh", Some(4)),
    ("video", "keep", Some(2)),
    ("gate", "fresh", Some(8)),
    ("helper", "keep", Some(2)),
    ("transliterator", "keep", Some(4)),
    ("upkeep", "fresh", None),
  ]
  .into_iter()
  .enumerate()
  {
    scratch
      .run(
        r#"INSERT INTO "agent_roles" ("name", "label", "bundles", "max_concurrent", "position", "created_at",
             "updated_at", "sessions") VALUES ($1, $1, '[]', $2, $3, 0, 0, $4)"#,
        vec![name.into(), cap.into(), (place as i64).into(), sessions.into()],
      )
      .await;
    scratch
      .run(r#"INSERT INTO "agent_role_seeded" ("name") VALUES ($1)"#, vec![name.into()])
      .await;
  }

  for (name, pooled) in [
    ("builder", 0i64),
    ("verifier", 1),
    ("digest", 1),
    ("scout", 1),
    ("screener", 1),
    ("transliterator", 1),
  ] {
    scratch
      .run(
        r#"INSERT INTO "coworker_pool_roles" ("name", "description", "prompt", "tools", "skills", "enabled",
             "pooled", "set_at") VALUES ($1, $2, $3, '[]', '[]', 1, $4, 0)"#,
        vec![
          name.into(),
          format!("What a {name} is for.").into(),
          format!("You are the {name}.\n\nModel settings: the tiers decide.").into(),
          pooled.into(),
        ],
      )
      .await;
  }

  scratch
    .run(
      r#"INSERT INTO "coworker_pool_tier_limits" ("provider", "tier", "capacity", "set_at")
         VALUES ('claude-code', 'Light', 12, 0)"#,
      vec![],
    )
    .await;
}

async fn row(scratch: &Scratch, table: &str, name: &str) -> sea_orm::QueryResult {
  store::one(
    scratch.db(),
    &format!(r#"SELECT * FROM "{table}" WHERE "name" = $1"#),
    vec![name.into()],
  )
  .await
  .unwrap()
  .unwrap_or_else(|| panic!("no {table} row {name}"))
}

fn order(row: &sea_orm::QueryResult) -> Vec<(String, String)> {
  let text: String = row.try_get("", "provider_order").unwrap();
  let entries: Vec<Value> = serde_json::from_str(&text).unwrap();

  entries
    .iter()
    .map(|entry| {
      (
        entry["provider"].as_str().unwrap().to_owned(),
        entry["model"].as_str().unwrap().to_owned(),
      )
    })
    .collect()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
  list
    .iter()
    .map(|(provider, model)| ((*provider).to_owned(), (*model).to_owned()))
    .collect()
}

async fn names(scratch: &Scratch, sql: &str) -> Vec<String> {
  store::all(scratch.db(), sql, vec![])
    .await
    .unwrap()
    .iter()
    .map(|row| row.try_get::<String>("", "name").unwrap())
    .collect()
}

#[tokio::test]
async fn chat_rows_become_supervisors_each_with_its_own_order_warming_and_threshold() {
  let scratch = before().await;

  live(&scratch).await;
  Migrator::up(scratch.db(), None).await.unwrap();

  assert_eq!(
    names(&scratch, r#"SELECT "name" FROM "supervisors" ORDER BY "name""#).await,
    ["bmo", "companion"]
  );

  let chat = row(&scratch, "supervisors", "bmo").await;
  // The route's provider first, then the owner's order, each on the Supervisor tier's model there.
  assert_eq!(
    order(&chat),
    pairs(&[
      ("deepseek", "deepseek-flash"),
      ("registered", "flash-next"),
      ("claude-code", "claude-opus-5-5"),
      ("codex", "gpt-5.6-sol"),
      ("openrouter", "nemotron"),
    ])
  );
  let entries: Vec<Value> =
    serde_json::from_str(&chat.try_get::<String>("", "provider_order").unwrap()).unwrap();
  assert_eq!(
    entries[0]["effort"], "max",
    "the supervisor's own effort stands on every entry taking a level"
  );
  assert_eq!(entries[1]["effort"], Value::Null, "a registered model takes no level");
  assert_eq!(entries[1]["thinking"], "on");
  assert_eq!(chat.try_get::<f64>("", "fold_ratio").unwrap(), 0.5);
  assert_eq!(chat.try_get::<f64>("", "quota_share").unwrap(), 0.01);
  assert_eq!(chat.try_get::<i64>("", "default_answerer").unwrap(), 1);
  let warming: Value = serde_json::from_str(&chat.try_get::<String>("", "warming").unwrap()).unwrap();
  assert_eq!(warming["deepseek"], true);

  let companion = row(&scratch, "supervisors", "companion").await;
  assert_eq!(
    order(&companion),
    pairs(&[("registered", "flash-next")]),
    "a fixed model is the whole order"
  );
  assert_eq!(companion.try_get::<f64>("", "fold_ratio").unwrap(), 0.95);
  assert_eq!(
    companion.try_get::<Option<String>>("", "repository").unwrap().as_deref(),
    Some("companion-repo")
  );
  let warming: Value = serde_json::from_str(&companion.try_get::<String>("", "warming").unwrap()).unwrap();
  assert_eq!(
    warming["registered"], false,
    "a supervisor whose keepalive was stopped is warmed nowhere"
  );
}

#[tokio::test]
async fn worker_rows_become_repositories_and_only_a_registered_pin_is_carried() {
  let scratch = before().await;

  live(&scratch).await;
  Migrator::up(scratch.db(), None).await.unwrap();

  assert_eq!(
    names(&scratch, r#"SELECT "name" FROM "repositories" ORDER BY "name""#).await,
    ["alpha", "companion-repo", "video", "web"],
    "every worker row is a repository but a support checkout and the helper's"
  );

  let alpha = row(&scratch, "repositories", "alpha").await;
  assert_eq!(alpha.try_get::<String>("", "path").unwrap(), "/repos/alpha");
  assert_eq!(
    alpha.try_get::<Option<String>>("", "pinned_provider").unwrap(),
    None,
    "a DeepSeek pin is not carried"
  );
  assert_eq!(
    alpha.try_get::<Option<String>>("", "preamble").unwrap().as_deref(),
    Some("alpha preamble")
  );
  assert_eq!(alpha.try_get::<i64>("", "downgrade_heal").unwrap(), 0);

  let pinned = row(&scratch, "repositories", "companion-repo").await;
  assert_eq!(
    pinned.try_get::<Option<String>>("", "pinned_provider").unwrap().as_deref(),
    Some("registered")
  );
  assert_eq!(
    pinned.try_get::<Option<String>>("", "pinned_model").unwrap().as_deref(),
    Some("flash-next")
  );

  let web = row(&scratch, "repositories", "web").await;
  assert_eq!(
    web.try_get::<i64>("", "support_count").unwrap(),
    1,
    "a support row here is a support checkout"
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "repository_node_checkouts" WHERE "repository" = 'web' AND "node" = 'n3'"#,
        vec![]
      )
      .await,
    1,
    "a support row on a node is the repository's checkout there"
  );
}

#[tokio::test]
async fn roles_take_their_orders_caps_and_lines_and_the_chat_and_gate_roles_go() {
  let scratch = before().await;

  live(&scratch).await;
  Migrator::up(scratch.db(), None).await.unwrap();

  assert_eq!(
    names(&scratch, r#"SELECT "name" FROM "roles" ORDER BY "position", "name""#).await,
    [
      "worker",
      "helper",
      "video",
      "builder",
      "verifier",
      "digest",
      "scout",
      "screener",
      "searcher",
      "browser",
      "translator",
      "transliterator",
      "upkeep"
    ]
  );

  let worker = row(&scratch, "roles", "worker").await;
  assert_eq!(order(&worker)[0], ("deepseek".to_owned(), "deepseek-flash".to_owned()));
  assert_eq!(worker.try_get::<i64>("", "max_concurrent").unwrap(), 3);
  assert_eq!(worker.try_get::<String>("", "line").unwrap(), "worker");
  assert_eq!(worker.try_get::<f64>("", "fold_ratio").unwrap(), 0.5);

  let helper = row(&scratch, "roles", "helper").await;
  assert_eq!(
    order(&helper)[0],
    ("claude-code".to_owned(), "claude-fable".to_owned()),
    "its fixed model first"
  );
  assert_eq!(order(&helper)[1], ("deepseek".to_owned(), "deepseek-flash".to_owned()));
  assert_eq!(helper.try_get::<i64>("", "max_concurrent").unwrap(), 2);

  let translator = row(&scratch, "roles", "translator").await;
  assert_eq!(
    order(&translator),
    pairs(&[("registered", "hy-mt")]),
    "the translator keeps its one registration"
  );
  assert_eq!(translator.try_get::<String>("", "line").unwrap(), "translate");

  let browser = row(&scratch, "roles", "browser").await;
  assert_eq!(browser.try_get::<String>("", "line").unwrap(), "browse");
  assert_eq!(browser.try_get::<String>("", "sessions").unwrap(), "fresh");

  for (name, cap, callable) in [
    ("builder", 8, 0),
    ("verifier", 6, 1),
    ("digest", 6, 1),
    ("scout", 12, 1),
    ("screener", 12, 1),
    ("upkeep", 1, 0),
  ] {
    let role = row(&scratch, "roles", name).await;
    assert_eq!(role.try_get::<i64>("", "max_concurrent").unwrap(), cap, "{name}'s cap");
    assert_eq!(role.try_get::<i64>("", "callable").unwrap(), callable, "whether {name} is callable");
  }

  let scout = row(&scratch, "roles", "scout").await;
  assert_eq!(
    scout.try_get::<Option<String>>("", "prompt").unwrap().as_deref(),
    Some("You are the scout.\n")
  );
  assert_eq!(
    scout.try_get::<Option<String>>("", "description").unwrap().as_deref(),
    Some("What a scout is for.")
  );
  assert_eq!(
    order(&scout)[0],
    ("deepseek".to_owned(), "deepseek-flash".to_owned()),
    "the owner's order"
  );
  let transliterator = row(&scratch, "roles", "transliterator").await;
  assert_eq!(
    transliterator.try_get::<i64>("", "callable").unwrap(),
    0,
    "a role already held stays uncallable"
  );

  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "role_seeded" WHERE "name" IN ('builder', 'browser')"#, vec![])
      .await,
    2,
    "a role the migration gave is recorded as given, so no seed gives it again"
  );
}

#[tokio::test]
async fn the_retired_tables_and_settings_go_and_the_owners_switches_stay() {
  let scratch = before().await;

  live(&scratch).await;
  Migrator::up(scratch.db(), None).await.unwrap();

  for table in [
    "agents",
    "model_tier_mappings",
    "fold_ratios",
    "fallback_routes",
    "coworker_pool_tier_limits",
  ] {
    assert_eq!(
      scratch
        .count(
          "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = current_schema() AND table_name = $1",
          vec![table.into()]
        )
        .await,
      0,
      "{table} is gone"
    );
  }

  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "settings" WHERE "key" = 'provider_order'"#, vec![])
      .await,
    0
  );
  let switch = store::one(
    scratch.db(),
    r#"SELECT "value" FROM "settings" WHERE "key" = 'provider_switch:codex'"#,
    vec![],
  )
  .await
  .unwrap()
  .unwrap();
  assert_eq!(switch.try_get::<String>("", "value").unwrap(), "off");
}

#[tokio::test]
async fn an_empty_store_carries_nothing() {
  let scratch = before().await;

  Migrator::up(scratch.db(), None).await.unwrap();

  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "supervisors""#, vec![]).await, 0);
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "repositories""#, vec![]).await, 0);
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "roles""#, vec![]).await, 0);
}
