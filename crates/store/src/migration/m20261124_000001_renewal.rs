//! The two axes (docs/agents.md, 「Two axes」): the supervisors, the roles and the repositories become tables
//! of their own, every supervisor and role carries its own provider order, a role is the cap of its slots,
//! and the agent rows, the tiers, the work keys' roads, the per-tier fold ratios and the co-worker pool's
//! capacities go. A store that held rows has them carried over as the owner settled it on 2026-11-02:
//!
//! - a chat row becomes a supervisor: its order the provider its route named first, then the owner's order,
//!   each on the model its tier mapped there; a chat row on a fixed model runs on that model alone;
//! - a worker row becomes a repository, a support row a support checkout of its repository; only a pin on a
//!   registered model is carried, and the job worker's route stands first in the worker role's order;
//! - the helper row's fixed model heads the helper role's order, a runner row becomes the role of its line,
//!   a co-worker definition becomes a callable role at its old tier limit, and the gate and chat roles go;
//! - every row's compaction threshold is the ratio its first provider held at its tier.
//!
//! An empty store carries nothing; the seeds give it the policy's roles as they settle it.

use std::collections::{BTreeMap, BTreeSet};

use sea_orm::{ConnectionTrait, QueryResult, Value as Bound};
use sea_orm_migration::prelude::*;
use serde_json::{Value, json};

use crate::statements::{all, one, run};

pub const NAME: &str = "m20261124_000001_renewal";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let held = Held::read(db).await.map_err(fault)?;

    db.execute_unprepared(include_str!("renewal_tables.sql")).await?;
    carry(db, &held).await.map_err(fault)?;
    db.execute_unprepared(include_str!("renewal_retired.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}

fn fault(fault: crate::Fault) -> DbErr {
  DbErr::Migration(fault.detail)
}

// The providers in the order a store that never saved one reads them.
const PROVIDERS: [&str; 5] = ["claude-code", "codex", "openrouter", "registered", "deepseek"];

// The provider whose models are the registry's registrations: it takes no effort level, and its tier
// mappings name thinking as their effort word.
const REGISTERED: &str = "registered";

// The roles the owner's decisions of 2026-11-02 set, by name: the cap each starts at, whether a run may call
// it as a co-worker, and the sessions setting a carried co-worker takes.
const CAPS: [(&str, i64); 7] = [
  ("worker", 3),
  ("builder", 8),
  ("verifier", 6),
  ("digest", 6),
  ("scout", 12),
  ("screener", 12),
  ("upkeep", 1),
];

// The order a screen lists the roles in, as the roles policy declares them.
const POSITIONS: [&str; 14] = [
  "worker",
  "delegator",
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
  "upkeep",
];

// The roles a runner row's line becomes, by the runner's name, with the work key its route was read under.
const RUNNERS: [(&str, &str, &str); 3] =
  [("search", "searcher", "search"), ("browse", "browser", "browse"), ("translator", "translator", "translate")];

// The line each carried role runs on, where it is not the worker line.
const LINES: [(&str, &str); 4] =
  [("searcher", "search"), ("browser", "browse"), ("translator", "translate"), ("transliterator", "search")];

// The work key each carried role's route and tier were read under.
const KEYS: [(&str, &str); 11] = [
  ("worker", "job-worker"),
  ("video", "job-worker"),
  ("helper", "role:helper"),
  ("builder", "role:builder"),
  ("verifier", "role:verifier"),
  ("digest", "role:digest"),
  ("scout", "role:scout"),
  ("screener", "role:screener"),
  ("searcher", "search"),
  ("browser", "browse"),
  ("transliterator", "role:transliterator"),
];

// The tier a role with no assignment of its own is read at.
const FALLBACK_TIER: &str = "Simple";

// The paragraph a co-worker definition carried about the retired tiers, which the role's prompt leaves out.
const TIER_NOTE: &str = "Model settings:";

/// One tier mapping: the model and effort a tier's work went out on at a provider.
#[derive(Clone, Debug)]
struct Mapping {
  model: String,
  effort: Option<String>,
  thinking: Option<bool>,
  answer_tokens: Option<i64>,
}

/// What the retired tables held, read before anything moves.
#[derive(Default)]
struct Held {
  carried: bool,
  order: Vec<(String, bool)>,
  saved_order: bool,
  mappings: BTreeMap<(String, String), Mapping>,
  assignments: BTreeMap<String, String>,
  ratios: BTreeMap<(String, String), f64>,
  routes: BTreeMap<String, (Option<String>, Option<String>)>,
  agents: Vec<QueryResult>,
  coworkers: Vec<QueryResult>,
}

fn text(row: &QueryResult, column: &str) -> Option<String> {
  row
    .try_get::<Option<String>>("", column)
    .ok()
    .flatten()
    .filter(|value| !value.trim().is_empty())
}

fn number(row: &QueryResult, column: &str) -> Option<i64> {
  row.try_get::<Option<i64>>("", column).ok().flatten()
}

impl Held {
  async fn read<C: ConnectionTrait>(db: &C) -> Result<Self, crate::Fault> {
    let agents = all(db, r#"SELECT * FROM "agents" ORDER BY "name""#, vec![]).await?;

    if agents.is_empty() {
      return Ok(Self::default());
    }

    let saved = one(db, r#"SELECT "value" FROM "settings" WHERE "key" = 'provider_order'"#, vec![])
      .await?
      .and_then(|row| text(&row, "value"));
    let order = saved_order(saved.as_deref());
    let mut held = Self {
      carried: true,
      saved_order: saved.is_some(),
      order,
      agents,
      ..Self::default()
    };

    for row in all(db, r#"SELECT * FROM "model_tier_mappings""#, vec![]).await? {
      let (Some(provider), Some(tier), Some(model)) = (text(&row, "provider"), text(&row, "tier"), text(&row, "model"))
      else {
        continue;
      };

      held.mappings.insert(
        (provider, tier),
        Mapping {
          model,
          effort: text(&row, "effort"),
          thinking: number(&row, "thinking").map(|on| on != 0),
          answer_tokens: number(&row, "answer_tokens"),
        },
      );
    }

    for row in all(db, r#"SELECT "key", "tier" FROM "model_tier_assignments""#, vec![]).await? {
      if let (Some(key), Some(tier)) = (text(&row, "key"), text(&row, "tier")) {
        held.assignments.insert(key, tier);
      }
    }

    for row in all(db, r#"SELECT "provider", "tier", "ratio" FROM "fold_ratios""#, vec![]).await? {
      if let (Some(provider), Some(tier), Ok(ratio)) =
        (text(&row, "provider"), text(&row, "tier"), row.try_get::<f64>("", "ratio"))
      {
        held.ratios.insert((provider, tier), ratio);
      }
    }

    for row in all(db, r#"SELECT "key", "route_mode", "registration" FROM "fallback_routes""#, vec![]).await? {
      if let Some(key) = text(&row, "key") {
        held.routes.insert(key, (text(&row, "route_mode"), text(&row, "registration")));
      }
    }

    held.coworkers = all(db, r#"SELECT * FROM "coworker_pool_roles" ORDER BY "name""#, vec![]).await?;

    Ok(held)
  }

  fn tier(&self, key: &str) -> String {
    self.assignments.get(key).cloned().unwrap_or_else(|| FALLBACK_TIER.to_owned())
  }

  // One entry of an order: the provider and what a tier's work went out on there.
  fn entry(&self, provider: &str, tier: &str) -> Option<Value> {
    let mapping = self.mappings.get(&(provider.to_owned(), tier.to_owned()))?;

    Some(entry_of(provider, &mapping.model, mapping.effort.as_deref(), mapping.thinking, mapping.answer_tokens))
  }

  // The order a row carries: its head first where it has one, then every provider of the owner's order the
  // tier mapped a model on, each once.
  fn order(&self, tier: &str, head: Option<Value>) -> Vec<Value> {
    let mut order: Vec<Value> = head.into_iter().collect();

    for (provider, _) in &self.order {
      if order.iter().any(|entry| entry["provider"] == provider.as_str()) {
        continue;
      }

      if let Some(entry) = self.entry(provider, tier) {
        order.push(entry);
      }
    }

    order
  }

  // The head a work key's route named: the provider it was set to, on the registration it named where it
  // named one, else on the tier's mapping there.
  fn route_head(&self, key: &str, tier: &str) -> Option<Value> {
    let (mode, registration) = self.routes.get(key)?;
    let mode = mode.as_deref().filter(|mode| PROVIDERS.contains(mode))?;

    match registration {
      Some(registration) if mode == REGISTERED => {
        let mapping = self.mappings.get(&(mode.to_owned(), tier.to_owned()));

        Some(entry_of(
          mode,
          registration,
          mapping.and_then(|mapping| mapping.effort.as_deref()),
          mapping.and_then(|mapping| mapping.thinking),
          mapping.and_then(|mapping| mapping.answer_tokens),
        ))
      }
      _ => self.entry(mode, tier),
    }
  }

  // The ratio the order's first provider held at the tier; 0.85 where none was kept.
  fn ratio(&self, order: &[Value], tier: &str) -> f64 {
    order
      .first()
      .and_then(|first| first["provider"].as_str())
      .and_then(|provider| self.ratios.get(&(provider.to_owned(), tier.to_owned())))
      .copied()
      .unwrap_or(0.85)
  }
}

// A saved order as it reads, each provider once with its switch, every provider it does not name after it,
// on; one that reads as nothing is every provider, on.
fn saved_order(saved: Option<&str>) -> Vec<(String, bool)> {
  let mut order: Vec<(String, bool)> = vec![];

  if let Some(Value::Array(entries)) = saved.and_then(|text| serde_json::from_str::<Value>(text).ok()) {
    for entry in entries {
      let read = match &entry {
        Value::String(name) => Some((name.clone(), true)),
        Value::Object(_) => entry["name"]
          .as_str()
          .map(|name| (name.to_owned(), entry["enabled"].as_bool().unwrap_or(true))),
        _ => None,
      };

      if let Some((name, enabled)) = read
        && PROVIDERS.contains(&name.as_str())
        && !order.iter().any(|(held, _)| *held == name)
      {
        order.push((name, enabled));
      }
    }
  }

  for provider in PROVIDERS {
    if !order.iter().any(|(held, _)| held == provider) {
      order.push((provider.to_owned(), true));
    }
  }

  order
}

// An order's entry, in the shape a row keeps it (docs/api.md, 「supervisors」): a registered model takes no
// effort level, its mapping's effort word naming whether it thinks.
fn entry_of(
  provider: &str,
  model: &str,
  effort: Option<&str>,
  thinking: Option<bool>,
  answer_tokens: Option<i64>,
) -> Value {
  let (effort, thinking) = if provider == REGISTERED {
    let thinks = match effort {
      Some("thinking") => Some(true),
      Some("no-thinking") => Some(false),
      _ => thinking,
    };

    (None, thinks)
  } else {
    (effort.map(str::to_owned), thinking)
  };

  json!({
    "provider": provider,
    "model": model,
    "effort": effort,
    "thinking": thinking.map(|on| if on { "on" } else { "off" }),
    "answerTokens": if provider == REGISTERED { answer_tokens } else { None },
  })
}

fn listed(value: &[(&str, &str)], name: &str) -> Option<String> {
  value.iter().find(|(key, _)| *key == name).map(|(_, value)| (*value).to_owned())
}

async fn carry<C: ConnectionTrait>(db: &C, held: &Held) -> Result<(), crate::Fault> {
  if !held.carried {
    return Ok(());
  }

  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|since| since.as_millis() as i64)
    .unwrap_or_default();

  switches(db, held).await?;
  supervisors(db, held, now).await?;
  repositories(db, held, now).await?;
  roles(db, held, now).await?;

  Ok(())
}

// The owner's switch on each provider, which the global order kept beside it, stands as a setting of its
// own (docs/provider-order.md, 「The owner's switch on a provider」).
async fn switches<C: ConnectionTrait>(db: &C, held: &Held) -> Result<(), crate::Fault> {
  if !held.saved_order {
    return Ok(());
  }

  for (provider, enabled) in &held.order {
    run(
      db,
      r#"INSERT INTO "settings" ("key", "value") VALUES ($1, $2)
         ON CONFLICT ("key") DO UPDATE SET "value" = EXCLUDED."value""#,
      vec![format!("provider_switch:{provider}").into(), (if *enabled { "on" } else { "off" }).into()],
    )
    .await?;
  }

  Ok(())
}

async fn supervisors<C: ConnectionTrait>(db: &C, held: &Held, now: i64) -> Result<(), crate::Fault> {
  for row in held.agents.iter().filter(|row| text(row, "kind").as_deref() == Some("chat")) {
    let Some(name) = text(row, "name") else { continue };
    let tier = held.assignments.get(&name).cloned().unwrap_or_else(|| "Supervisor".to_owned());
    let effort = text(row, "effort");
    let thinking = number(row, "thinking").map(|on| on != 0);
    let pin = text(row, "pinned_provider").zip(text(row, "pinned_model"));
    let order = match &pin {
      // A supervisor on a fixed model runs on that model alone.
      Some((provider, model)) => {
        let mapping = held.mappings.get(&(provider.clone(), tier.clone()));
        let effort = effort.clone().or_else(|| mapping.and_then(|mapping| mapping.effort.clone()));

        vec![entry_of(
          provider,
          model,
          effort.as_deref(),
          thinking.or_else(|| mapping.and_then(|mapping| mapping.thinking)),
          mapping.and_then(|mapping| mapping.answer_tokens),
        )]
      }
      None => {
        let mut order = held.order(&tier, held.route_head(&name, &tier));

        // The supervisor's own effort stands on every entry that takes a level.
        if let Some(effort) = &effort {
          for entry in &mut order {
            if entry["provider"] != REGISTERED {
              entry["effort"] = json!(effort);
            }
          }
        }

        order
      }
    };
    let warm = number(row, "keepalive_stopped").unwrap_or_default() == 0;
    let warming: serde_json::Map<String, Value> = order
      .iter()
      .filter_map(|entry| entry["provider"].as_str())
      .map(|provider| (provider.to_owned(), json!(warm)))
      .collect();
    let fold_ratio = held.ratio(&order, &tier);

    run(
      db,
      r#"INSERT INTO "supervisors" ("name", "label", "owner_account", "aliases", "wake_words", "icon", "prompts",
           "brain_folder", "voice", "window_seconds", "session_scope", "turn_shape", "default_answerer", "repository",
           "provider_order", "quota_share", "warming", "fold_ratio", "builtin", "chat_id", "session_id", "last_mode",
           "last_model", "last_effort", "last_run_at", "last_keepalive_at", "runs", "context_tokens",
           "prompt_changed_at", "herdr_label", "created_at", "updated_at")
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, 0.01, $16, $17, $18, $19, $20,
           $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $30)"#,
      vec![
        name.clone().into(),
        text(row, "label").into(),
        number(row, "owner_account").into(),
        text(row, "aliases").into(),
        text(row, "wake_words").into(),
        text(row, "icon").into(),
        text(row, "prompts").into(),
        text(row, "brain_folder").into(),
        text(row, "voice").into(),
        number(row, "window_seconds").into(),
        text(row, "session_scope").unwrap_or_else(|| "agent".to_owned()).into(),
        text(row, "turn_shape").unwrap_or_else(|| "window".to_owned()).into(),
        number(row, "default_answerer").unwrap_or_default().into(),
        text(row, "item_worker").into(),
        Value::Array(order).to_string().into(),
        Value::Object(warming).to_string().into(),
        fold_ratio.into(),
        number(row, "builtin").unwrap_or_default().into(),
        number(row, "chat_id").into(),
        text(row, "session_id").into(),
        text(row, "last_mode").into(),
        text(row, "last_model").into(),
        text(row, "last_effort").into(),
        number(row, "last_run_at").into(),
        number(row, "last_keepalive_at").into(),
        number(row, "runs").unwrap_or_default().into(),
        number(row, "context_tokens").into(),
        number(row, "prompt_changed_at").into(),
        text(row, "herdr_label").into(),
        now.into(),
      ],
    )
    .await?;
  }

  Ok(())
}

// Whether a worker row is a repository's own: a worker row standing for no role's carrier, neither a support
// checkout nor the helper's.
fn is_repository(row: &QueryResult) -> bool {
  text(row, "kind").as_deref() == Some("worker")
    && text(row, "support_of").is_none()
    && text(row, "role").as_deref() != Some("helper")
}

async fn repositories<C: ConnectionTrait>(db: &C, held: &Held, now: i64) -> Result<(), crate::Fault> {
  let names: BTreeSet<String> =
    held.agents.iter().filter(|row| is_repository(row)).filter_map(|row| text(row, "name")).collect();

  for row in held.agents.iter().filter(|row| is_repository(row)) {
    let Some(name) = text(row, "name") else { continue };
    let supports = held
      .agents
      .iter()
      .filter(|support| {
        text(support, "kind").as_deref() == Some("worker")
          && text(support, "support_of").as_deref() == Some(name.as_str())
          && text(support, "node").is_none()
      })
      .count() as i64;
    let support_count = number(row, "support_count").unwrap_or_default().max(supports);
    // Only a pin on a registered model is the repository's own (the owner's decision on Q11).
    let pin = text(row, "pinned_provider")
      .zip(text(row, "pinned_model"))
      .filter(|(provider, _)| provider == REGISTERED);

    run(
      db,
      r#"INSERT INTO "repositories" ("name", "label", "owner_account", "path", "node", "support_count",
           "prefix_documents", "preamble", "prompts", "dev_command", "pinned_provider", "pinned_model",
           "downgrade_heal", "created_at", "updated_at")
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $14)"#,
      vec![
        name.clone().into(),
        text(row, "label").into(),
        number(row, "owner_account").into(),
        text(row, "cwd").unwrap_or_default().into(),
        text(row, "node").into(),
        support_count.into(),
        text(row, "prefix_documents").into(),
        text(row, "preamble").into(),
        text(row, "prompts").into(),
        text(row, "dev_command").into(),
        pin.as_ref().map(|(provider, _)| provider.clone()).into(),
        pin.as_ref().map(|(_, model)| model.clone()).into(),
        number(row, "downgrade_heal").unwrap_or(1).into(),
        now.into(),
      ],
    )
    .await?;
  }

  // A support row standing on a node is that repository's checkout there.
  for row in held.agents.iter() {
    let (Some(of), Some(node), Some(path)) = (text(row, "support_of"), text(row, "node"), text(row, "cwd")) else {
      continue;
    };

    if !names.contains(&of) {
      continue;
    }

    run(
      db,
      r#"INSERT INTO "repository_node_checkouts" ("repository", "node", "path", "created_at")
         VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING"#,
      vec![of.into(), node.into(), path.into(), now.into()],
    )
    .await?;
  }

  Ok(())
}

// The order a carried role runs on.
fn role_order(held: &Held, role: &str) -> (Vec<Value>, String) {
  let key = listed(&KEYS, role)
    .or_else(|| RUNNERS.iter().find(|(_, held_role, _)| *held_role == role).map(|(_, _, key)| (*key).to_owned()));
  let tier = key.as_deref().map(|key| held.tier(key)).unwrap_or_else(|| FALLBACK_TIER.to_owned());

  // The worker role's first entry is DeepSeek's flash model (the owner's decision on Q11).
  if role == "worker" {
    let mapping = held.mappings.get(&("deepseek".to_owned(), tier.clone()));
    let head = entry_of(
      "deepseek",
      "deepseek-flash",
      mapping.and_then(|mapping| mapping.effort.as_deref()),
      mapping.and_then(|mapping| mapping.thinking),
      None,
    );

    return (held.order(&tier, Some(head)), tier);
  }

  // The translator runs on the one registration its route named, and on no other provider.
  if role == "translator"
    && let Some(head) = key.as_deref().and_then(|key| held.route_head(key, &tier))
  {
    return (vec![head], tier);
  }

  // A role whose carrier row kept a fixed model starts its order on it.
  let pinned = held
    .agents
    .iter()
    .find(|row| text(row, "role").as_deref() == Some(role) && text(row, "kind").as_deref() == Some("worker"))
    .filter(|row| text(row, "name").as_deref() == Some(role))
    .and_then(|row| {
      let (provider, model) = text(row, "pinned_provider").zip(text(row, "pinned_model"))?;
      let mapping = held.mappings.get(&(provider.clone(), tier.clone()));
      let effort = text(row, "effort").or_else(|| mapping.and_then(|mapping| mapping.effort.clone()));

      Some(entry_of(
        &provider,
        &model,
        effort.as_deref(),
        number(row, "thinking").map(|on| on != 0).or_else(|| mapping.and_then(|mapping| mapping.thinking)),
        mapping.and_then(|mapping| mapping.answer_tokens),
      ))
    });
  let head = pinned.or_else(|| key.as_deref().and_then(|key| held.route_head(key, &tier)));

  (held.order(&tier, head), tier)
}

async fn roles<C: ConnectionTrait>(db: &C, held: &Held, now: i64) -> Result<(), crate::Fault> {
  let web = r#"["web.read","web.sites","web.branches","web.rulebooks"]"#;

  // A runner's line becomes its own role where the store holds none of it.
  for (runner, role, _) in RUNNERS {
    if held.agents.iter().any(|row| text(row, "name").as_deref() == Some(runner)) {
      run(
        db,
        r#"INSERT INTO "roles" ("name", "label", "bundles", "max_concurrent", "position", "created_at",
             "updated_at", "sessions")
           VALUES ($1, $2, $3, 4, 0, $4, $4, 'fresh') ON CONFLICT ("name") DO NOTHING"#,
        vec![role.into(), capitalised(role).into(), web.into(), now.into()],
      )
      .await?;
    }
  }

  // A co-worker definition becomes a role of its own name: its description, and its prompt without what it
  // said of the tiers.
  let before: BTreeSet<String> = all(db, r#"SELECT "name" FROM "roles""#, vec![])
    .await?
    .iter()
    .filter_map(|row| text(row, "name"))
    .collect();

  for row in &held.coworkers {
    let Some(name) = text(row, "name") else { continue };
    let prompt = text(row, "prompt").map(|prompt| without_tier_note(&prompt));
    let description = text(row, "description");
    let sessions = if name == "builder" { "keep" } else { "fresh" };

    run(
      db,
      r#"INSERT INTO "roles" ("name", "label", "bundles", "max_concurrent", "prompt", "position", "created_at",
           "updated_at", "sessions", "description")
         VALUES ($1, $2, '["graph.read","queue.read","web.read"]', 4, $3, 0, $4, $4, $5, $6)
         ON CONFLICT ("name") DO UPDATE SET "description" = COALESCE("roles"."description", EXCLUDED."description")"#,
      vec![name.clone().into(), capitalised(&name).into(), prompt.into(), now.into(), sessions.into(), description.into()],
    )
    .await?;

    // A definition the pool lent out becomes a callable role, the builder aside: it is the work order's own turn.
    if !before.contains(&name) {
      let callable = name != "builder" && number(row, "pooled").unwrap_or(1) != 0;

      run(
        db,
        r#"UPDATE "roles" SET "callable" = $2 WHERE "name" = $1"#,
        vec![name.clone().into(), i64::from(callable).into()],
      )
      .await?;
    }
  }

  let names: Vec<String> = all(db, r#"SELECT "name" FROM "roles""#, vec![])
    .await?
    .iter()
    .filter_map(|row| text(row, "name"))
    .collect();

  for name in &names {
    let (order, tier) = role_order(held, name);
    let line = listed(&LINES, name).unwrap_or_else(|| "worker".to_owned());
    let fold_ratio = held.ratio(&order, &tier);
    let position = POSITIONS.iter().position(|one| one == name).unwrap_or(POSITIONS.len()) as i64;
    let mut values: Vec<Bound> = vec![
      name.clone().into(),
      Value::Array(order).to_string().into(),
      line.into(),
      fold_ratio.into(),
      position.into(),
      now.into(),
    ];
    let cap = CAPS.iter().find(|(role, _)| role == name).map(|(_, cap)| *cap);

    values.push(cap.into());
    run(
      db,
      r#"UPDATE "roles" SET "provider_order" = $2, "line" = $3, "fold_ratio" = $4, "position" = $5,
           "updated_at" = $6, "max_concurrent" = COALESCE($7, "max_concurrent", 1)
         WHERE "name" = $1"#,
      values,
    )
    .await?;
    run(
      db,
      r#"INSERT INTO "role_seeded" ("name") VALUES ($1) ON CONFLICT DO NOTHING"#,
      vec![name.clone().into()],
    )
    .await?;
  }

  Ok(())
}

fn capitalised(name: &str) -> String {
  let mut letters = name.chars();

  letters
    .next()
    .map(|first| first.to_uppercase().chain(letters).collect())
    .unwrap_or_default()
}

// A co-worker definition's prompt without its closing paragraph on the retired tiers.
fn without_tier_note(prompt: &str) -> String {
  match prompt.find(TIER_NOTE) {
    Some(at) => prompt[..at].trim_end().to_owned() + "\n",
    None => prompt.to_owned(),
  }
}
