//! The roles as the owner lists, adds, changes and removes them (docs/agent-roles.md, 「Changing the roles」;
//! docs/api.md, 「roles」): each a row of the store with its own provider order, cap, sessions setting,
//! compaction threshold and bundles. A field is judged before anything is written.

use std::sync::LazyLock;

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use policy::roles::{ROLES, Sessions};
use policy::run_lines::RUN_LINES;
use regex::Regex;
use sea_orm::ConnectionTrait;
use serde_json::{Value, json};

use super::claude_accounts::Judging;
use super::{Call, Refused, Route, answer, whole};
use crate::agent_roles::{self, AgentRole};
use crate::now_ms;
use crate::order::{self, Order};
use crate::order_run::dispatch;
use crate::places::Ground;
use crate::road::{self, Ask, Row};
use crate::text::length;
use store::{all, one, run};

static NAME: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(&ROLES.name_pattern).expect("the policy's role name pattern reads"));

// The lines a role may name: every line agency carries but the supervisors' and the gate's.
const NOT_A_ROLES_LINE: [&str; 2] = ["chat", "gate"];

fn unprocessable(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::UNPROCESSABLE_ENTITY, error)
}

fn unknown(name: &str) -> Refused {
  Refused::new(StatusCode::NOT_FOUND, fill_marks(&CONTROL_WORDS.agent_roles.unknown, &[("name", name)]))
}

fn label_refusal() -> String {
  fill_marks(&CONTROL_WORDS.agent_roles.label, &[("most", &ROLES.label_most.to_string())])
}

fn bundles_refusal() -> String {
  let declared = ROLES.bundles.iter().map(|bundle| bundle.name.as_str()).collect::<Vec<_>>();

  fill_marks(&CONTROL_WORDS.agent_roles.bundles, &[("bundles", &declared.join(", "))])
}

/// Every line a role may name.
pub fn role_lines() -> Vec<String> {
  RUN_LINES
    .0
    .keys()
    .filter(|line| !NOT_A_ROLES_LINE.contains(&line.as_str()))
    .cloned()
    .collect()
}

fn line_refusal() -> String {
  fill_marks(&CONTROL_WORDS.agent_roles.line, &[("lines", &role_lines().join(", "))])
}

/// A bundle as a screen is handed it: its name, its kebab-case code and its label.
pub fn bundle_view(name: &str) -> Value {
  json!({ "name": name, "code": name.replace('.', "-"), "label": ROLES.bundle_label(name) })
}

/// What stands of a role now: how many of its runs go and wait, its active provider and why its runs wait.
async fn standing(ground: &Ground, role: &AgentRole) -> Result<Value, Refused> {
  let count = |state: &'static str| {
    let db = &ground.db;
    let name = role.name.clone();

    async move {
      one(
        db,
        r#"SELECT COUNT(*) AS "count" FROM "role_slot_requests" WHERE "role" = $1 AND "state" = $2"#,
        vec![name.into(), state.into()],
      )
      .await
      .map(|row| row.and_then(|row| row.try_get::<i64>("", "count").ok()).unwrap_or(0))
    }
  };
  let running = count("active").await?;
  let waiting = count("waiting").await?;
  let resolution = road::resolve(ground, &Ask::of(Row::Role(&role.name), &role.line, &role.order))
    .await
    .map_err(Refused::failed)?;
  let provider = resolution.road().map(|road| road.provider.clone());
  let waiting_for = provider.is_none().then(|| resolution.sentence());

  Ok(json!({
    "running": running,
    "waiting": waiting,
    "provider": provider,
    "waitingFor": waiting_for,
  }))
}

/// One role as the routes answer it: every field of its row, and what stands of it now.
pub async fn view(ground: &Ground, role: &AgentRole) -> Result<Value, Refused> {
  let mut shown = json!({
    "name": role.name,
    "code": role.name,
    "label": role.label,
    "prompt": role.prompt,
    "description": role.description,
    "bundles": role.bundles.iter().map(|bundle| bundle_view(bundle)).collect::<Vec<_>>(),
    "line": role.line,
    "order": role.order.entries,
    "quotaShare": role.order.quota_share,
    "maxConcurrent": role.max_concurrent,
    "sessions": role.sessions.word(),
    "foldRatio": role.fold_ratio,
    "callable": role.callable,
  });

  if let (Some(shown), Value::Object(now)) = (shown.as_object_mut(), standing(ground, role).await?) {
    shown.extend(now);
  }

  Ok(shown)
}

/// What a body asks of a role's fields: each one it names, judged. A field left out is `None`; a prompt or a
/// description given as null is `Some(None)`.
#[derive(Default)]
struct Fields {
  label: Option<String>,
  bundles: Option<Vec<String>>,
  line: Option<String>,
  order: Option<Order>,
  bound: Option<i64>,
  prompt: Option<Option<String>>,
  description: Option<Option<String>>,
  sessions: Option<Sessions>,
  fold_ratio: Option<f64>,
  callable: Option<bool>,
}

fn optional_text(value: &Value, refusal: &str) -> Result<Option<String>, Refused> {
  match value {
    Value::Null => Ok(None),
    Value::String(text) if text.trim().is_empty() => Ok(None),
    Value::String(text) => Ok(Some(text.clone())),
    _ => Err(unprocessable(refusal)),
  }
}

impl Fields {
  // The fields a body names, judged; an order against the line the body names, else the one the role holds.
  async fn of(ground: &Ground, body: &Value, held_line: Option<&str>) -> Result<Self, Refused> {
    let words = &CONTROL_WORDS.agent_roles;
    let mut fields = Self::default();

    if let Some(label) = body.get("label") {
      match label.as_str().map(str::trim) {
        Some(label) if !label.is_empty() && length(label) <= ROLES.label_most => {
          fields.label = Some(label.to_owned());
        }
        _ => return Err(unprocessable(label_refusal())),
      }
    }

    if let Some(bundles) = body.get("bundles") {
      let named: Option<Vec<&str>> =
        bundles.as_array().and_then(|list| list.iter().map(Value::as_str).collect());
      let Some(named) = named.filter(|named| named.iter().all(|bundle| ROLES.declares_bundle(bundle)))
      else {
        return Err(unprocessable(bundles_refusal()));
      };
      let mut given: Vec<String> = Vec::with_capacity(named.len());

      for bundle in named {
        if !given.iter().any(|one| one == bundle) {
          given.push(bundle.to_owned());
        }
      }

      fields.bundles = Some(given);
    }

    if let Some(line) = body.get("line") {
      match line.as_str().filter(|line| role_lines().iter().any(|one| one == line)) {
        Some(line) => fields.line = Some(line.to_owned()),
        None => return Err(unprocessable(line_refusal())),
      }
    }

    if let Some(entries) = body.get("order") {
      let line = fields.line.as_deref().or(held_line).unwrap_or("worker");
      let judged =
        order::judge(Judging::of(ground), &ground.machine(), line, entries, body.get("quotaShare"))
          .await
          .map_err(|refusal| unprocessable(format!("{}: {}", refusal.field, refusal.error)))?;

      fields.order = Some(judged);
    }

    if let Some(bound) = body.get("maxConcurrent") {
      match whole(bound).filter(|bound| *bound >= 0) {
        Some(bound) => fields.bound = Some(bound),
        None => return Err(unprocessable(words.bound.as_str())),
      }
    }

    if let Some(prompt) = body.get("prompt") {
      fields.prompt = Some(optional_text(prompt, &words.prompt)?);
    }

    if let Some(description) = body.get("description") {
      fields.description = Some(optional_text(description, &words.description)?);
    }

    if let Some(sessions) = body.get("sessions") {
      match sessions.as_str().and_then(Sessions::of) {
        Some(sessions) => fields.sessions = Some(sessions),
        None => return Err(unprocessable(words.sessions.as_str())),
      }
    }

    if let Some(ratio) = body.get("foldRatio") {
      match ratio.as_f64().filter(|ratio| *ratio > 0.0 && *ratio <= 1.0) {
        Some(ratio) => fields.fold_ratio = Some(ratio),
        None => return Err(unprocessable(words.fold_ratio.as_str())),
      }
    }

    if let Some(callable) = body.get("callable") {
      match callable.as_bool() {
        Some(callable) => fields.callable = Some(callable),
        None => return Err(unprocessable(words.callable.as_str())),
      }
    }

    Ok(fields)
  }
}

/// How many runs of a role go or wait now.
async fn holding<C: ConnectionTrait>(db: &C, role: &str) -> Result<i64, Refused> {
  Ok(
    one(
      db,
      r#"SELECT COUNT(*) AS "count" FROM "role_slot_requests"
         WHERE "role" = $1 AND "state" IN ('waiting', 'reserved', 'active')"#,
      vec![role.into()],
    )
    .await?
    .map(|row| row.try_get::<i64>("", "count"))
    .transpose()?
    .unwrap_or(0),
  )
}

/// `GET /roles`: every role the store holds, every bundle a role may give and every line a role may name.
pub struct RoleList;

impl Route for RoleList {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/roles";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let mut roles = vec![];

    for role in agent_roles::every(call.store()).await? {
      roles.push(view(call.ground(), &role).await?);
    }

    let bundles: Vec<Value> = ROLES.bundles.iter().map(|bundle| bundle_view(&bundle.name)).collect();

    Ok(answer(
      StatusCode::OK,
      json!({ "roles": roles, "bundles": bundles, "lines": role_lines() }),
    ))
  }
}

crate::moved!(RoleList);

/// `GET /roles/{roleName}`: one role, with how many of its runs go and wait.
pub struct RoleRead;

impl Route for RoleRead {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/roles/{roleName}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let name = call.param("roleName").unwrap_or_default().to_owned();
    let role = agent_roles::named(call.store(), &name).await?.ok_or_else(|| unknown(&name))?;

    Ok(answer(StatusCode::OK, view(call.ground(), &role).await?))
  }
}

crate::moved!(RoleRead);

/// `POST /roles`: a role added.
pub struct RoleCreate;

impl Route for RoleCreate {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/roles";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let words = &CONTROL_WORDS.agent_roles;
    let name = body.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();

    if !NAME.is_match(&name) || length(&name) > ROLES.name_most {
      return Err(unprocessable(fill_marks(&words.name, &[("most", &ROLES.name_most.to_string())])));
    }

    if agent_roles::holds(call.store(), &name).await? {
      return Err(unprocessable(fill_marks(&words.taken, &[("name", &name)])));
    }

    let fields = Fields::of(call.ground(), &body, None).await?;
    let label = fields.label.clone().ok_or_else(|| unprocessable(label_refusal()))?;
    let bundles = fields.bundles.clone().ok_or_else(|| unprocessable(bundles_refusal()))?;
    let line = fields.line.clone().ok_or_else(|| unprocessable(line_refusal()))?;
    let order = fields
      .order
      .clone()
      .ok_or_else(|| unprocessable("order: A role names its provider order."))?;
    let bound = fields.bound.ok_or_else(|| unprocessable(words.bound.as_str()))?;
    let now = now_ms();

    run(
      call.store(),
      r#"INSERT INTO "roles" ("name", "label", "bundles", "max_concurrent", "prompt", "position", "created_at",
           "updated_at", "sessions", "description", "line", "provider_order", "quota_share", "fold_ratio", "callable")
         VALUES ($1, $2, $3, $4, $5, (SELECT COALESCE(MAX("position"), -1) + 1 FROM "roles"), $6, $6, $7, $8, $9, $10,
           $11, $12, $13)"#,
      vec![
        name.clone().into(),
        label.into(),
        serde_json::to_string(&bundles).map_err(Refused::failed)?.into(),
        bound.into(),
        fields.prompt.flatten().into(),
        now.into(),
        fields.sessions.unwrap_or_default().word().into(),
        fields.description.flatten().into(),
        line.into(),
        order.entries_text().into(),
        order.quota_share.into(),
        fields.fold_ratio.unwrap_or_else(|| order.default_fold_ratio()).into(),
        i64::from(fields.callable.unwrap_or(false)).into(),
      ],
    )
    .await?;

    let role = agent_roles::named(call.store(), &name).await?.ok_or_else(|| unknown(&name))?;

    Ok(answer(StatusCode::CREATED, view(call.ground(), &role).await?))
  }
}

crate::moved!(RoleCreate);

/// `PATCH /roles/{roleName}`: any field of a role changed; a raised cap lets in at once the reserved slots it
/// frees.
pub struct RoleChange;

impl Route for RoleChange {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/roles/{roleName}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let name = call.param("roleName").unwrap_or_default().to_owned();
    let body = call.body().await?;
    let Some(held) = agent_roles::named(call.store(), &name).await? else {
      return Err(unknown(&name));
    };
    let fields = Fields::of(call.ground(), &body, Some(&held.line)).await?;
    let order = fields.order.unwrap_or(held.order);

    run(
      call.store(),
      r#"UPDATE "roles" SET "label" = $1, "bundles" = $2, "max_concurrent" = $3, "prompt" = $4, "updated_at" = $5,
           "sessions" = $7, "description" = $8, "line" = $9, "provider_order" = $10, "quota_share" = $11,
           "fold_ratio" = $12, "callable" = $13
         WHERE "name" = $6"#,
      vec![
        fields.label.unwrap_or(held.label).into(),
        serde_json::to_string(&fields.bundles.unwrap_or(held.bundles)).map_err(Refused::failed)?.into(),
        fields.bound.unwrap_or(held.max_concurrent).into(),
        fields.prompt.unwrap_or(held.prompt).into(),
        now_ms().into(),
        name.clone().into(),
        fields.sessions.unwrap_or(held.sessions).word().into(),
        fields.description.unwrap_or(held.description).into(),
        fields.line.unwrap_or(held.line).into(),
        order.entries_text().into(),
        order.quota_share.into(),
        fields.fold_ratio.unwrap_or(held.fold_ratio).into(),
        i64::from(fields.callable.unwrap_or(held.callable)).into(),
      ],
    )
    .await?;

    dispatch::promote_role(call.store(), &name, now_ms()).await.map_err(Refused::failed)?;

    let role = agent_roles::named(call.store(), &name).await?.ok_or_else(|| unknown(&name))?;

    Ok(answer(StatusCode::OK, view(call.ground(), &role).await?))
  }
}

crate::moved!(RoleChange);

/// `DELETE /roles/{roleName}`: a role no run holds or waits for and no default names, removed.
pub struct RoleDelete;

impl Route for RoleDelete {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/roles/{roleName}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let words = &CONTROL_WORDS.agent_roles;
    let name = call.param("roleName").unwrap_or_default().to_owned();

    if !agent_roles::holds(call.store(), &name).await? {
      return Err(unknown(&name));
    }

    if ROLES.default_role(&name) {
      return Err(Refused::new(StatusCode::CONFLICT, fill_marks(&words.default_role, &[("name", &name)])));
    }

    if holding(call.store(), &name).await? > 0 {
      return Err(Refused::new(StatusCode::CONFLICT, fill_marks(&words.held, &[("name", &name)])));
    }

    run(call.store(), r#"DELETE FROM "roles" WHERE "name" = $1"#, vec![name.into()]).await?;

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

crate::moved!(RoleDelete);

// The most sessions a history page holds.
const HISTORY_PAGE: i64 = 50;

/// `GET /roles/{roleName}/sessions`: the sessions the role's runs opened, newest first (docs/sessions.md,
/// 「The session history of a role」).
pub struct RoleSessions;

impl Route for RoleSessions {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/roles/{roleName}/sessions";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let name = call.param("roleName").unwrap_or_default().to_owned();

    if !agent_roles::holds(call.store(), &name).await? {
      return Err(unknown(&name));
    }

    let before = call.query("before").and_then(|before| before.parse::<i64>().ok()).unwrap_or(i64::MAX);
    let limit = call
      .query("limit")
      .and_then(|limit| limit.parse::<i64>().ok())
      .filter(|limit| *limit > 0)
      .unwrap_or(HISTORY_PAGE);
    let rows = all(
      call.store(),
      r#"SELECT * FROM "role_sessions" WHERE "role" = $1 AND "id" < $2 ORDER BY "id" DESC LIMIT $3"#,
      vec![name.into(), before.into(), (limit + 1).into()],
    )
    .await?;
    let more = i64::try_from(rows.len()).unwrap_or(i64::MAX) > limit;
    let mut items = vec![];

    for row in rows.iter().take(usize::try_from(limit).unwrap_or(usize::MAX)) {
      let held = crate::apart_sessions::Kept::of(row).map_err(Refused::failed)?;

      items.push(json!({
        "id": held.id,
        "sessionId": held.session,
        "path": held.path,
        "provider": held.provider,
        "model": held.model,
        "effort": (!held.effort.is_empty()).then_some(held.effort.clone()),
        "account": held.account,
        "openedAt": held.opened_at,
        "lastAnsweredAt": held.answered_at,
        "runs": held.runs,
        "context": { "tokens": held.context_tokens, "share": held.context_share },
        "valid": crate::apart_sessions::valid(&held, now_ms()),
      }));
    }

    Ok(answer(StatusCode::OK, json!({ "items": items, "more": more })))
  }
}

crate::moved!(RoleSessions);
