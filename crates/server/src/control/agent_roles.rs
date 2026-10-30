//! The roles an agent may hold, as the owner lists, adds, changes and removes them (docs/agent-roles.md,
//! 「Changing the roles」; docs/api.md, 「agent-roles」): each a row of the store, its tier the assignment
//! of its work key `role:<name>`, and the bundles it may give the policy's. A field is judged before
//! anything is written, and a role with its key's assignment is written in one transaction.

use std::sync::LazyLock;

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use policy::roles::ROLES;
use policy::tiers::TIERS;
use regex::Regex;
use sea_orm::{ConnectionTrait, TransactionTrait};
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer, tiers, whole};
use crate::agent_roles::{self, AgentRole};
use crate::now_ms;
use crate::text::length;
use store::{all, one, run};

static NAME: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(&ROLES.name_pattern).expect("the policy's role name pattern reads"));

fn unprocessable(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::UNPROCESSABLE_ENTITY, error)
}

fn unknown(name: &str) -> Refused {
  Refused::new(StatusCode::NOT_FOUND, fill_marks(&CONTROL_WORDS.agent_roles.unknown, &[("name", name)]))
}

/// The work key a role's tier is the assignment of.
pub fn key_of(role: &str) -> String {
  format!("role:{role}")
}

fn label_refusal() -> String {
  fill_marks(&CONTROL_WORDS.agent_roles.label, &[("most", &ROLES.label_most.to_string())])
}

fn bundles_refusal() -> String {
  let declared = ROLES.bundles.iter().map(|bundle| bundle.name.as_str()).collect::<Vec<_>>();

  fill_marks(&CONTROL_WORDS.agent_roles.bundles, &[("bundles", &declared.join(", "))])
}

fn bundle_view(name: &str) -> Value {
  json!({ "name": name, "label": ROLES.bundle_label(name) })
}

/// One role as the routes answer it: its row, its key's tier, how many agents hold it and whether a kind's
/// own is it.
async fn view<C: ConnectionTrait>(db: &C, role: &AgentRole) -> Result<Value, Refused> {
  let tier: Option<String> = one(
    db,
    r#"SELECT "tier" FROM "model_tier_assignments" WHERE "key" = $1"#,
    vec![key_of(&role.name).into()],
  )
  .await?
  .map(|row| row.try_get("", "tier"))
  .transpose()?;

  Ok(json!({
    "name": role.name,
    "label": role.label,
    "bundles": role.bundles.iter().map(|bundle| bundle_view(bundle)).collect::<Vec<_>>(),
    "maxConcurrent": role.max_concurrent,
    "prompt": role.prompt,
    "tier": tier,
    "agents": holders(db, &role.name).await?.len(),
    "kindDefault": ROLES.kind_default(&role.name),
  }))
}

/// The agents whose rows hold a role, in name order.
async fn holders<C: ConnectionTrait>(db: &C, role: &str) -> Result<Vec<String>, Refused> {
  all(
    db,
    r#"SELECT "name" FROM "agents" WHERE "role" = $1 ORDER BY "name""#,
    vec![role.into()],
  )
  .await?
  .iter()
  .map(|row| row.try_get("", "name").map_err(Refused::from))
  .collect()
}

/// What a body asks of a role's fields: each one it names, judged. A field left out is `None`; a bound, a
/// prompt or a tier given as null is `Some(None)`.
#[derive(Default)]
struct Fields {
  label: Option<String>,
  bundles: Option<Vec<String>>,
  bound: Option<Option<i64>>,
  prompt: Option<Option<String>>,
  tier: Option<Option<String>>,
}

impl Fields {
  fn of(body: &Value) -> Result<Self, Refused> {
    let words = &CONTROL_WORDS.agent_roles;
    let mut fields = Self::default();

    if let Some(label) = body.get("label") {
      match label.as_str().map(str::trim) {
        Some(label) if !label.is_empty() && length(label) <= ROLES.label_most => {
          fields.label = Some(label.to_owned());
        }
        _ => {
          return Err(unprocessable(label_refusal()));
        }
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

    if let Some(bound) = body.get("maxConcurrent") {
      fields.bound = Some(match bound {
        Value::Null => None,
        given => match whole(given).filter(|bound| *bound >= 1) {
          Some(bound) => Some(bound),
          None => return Err(unprocessable(words.bound.as_str())),
        },
      });
    }

    if let Some(prompt) = body.get("prompt") {
      fields.prompt = Some(match prompt {
        Value::Null => None,
        Value::String(text) if text.trim().is_empty() => None,
        Value::String(text) => Some(text.clone()),
        _ => return Err(unprocessable(words.prompt.as_str())),
      });
    }

    if let Some(tier) = body.get("tier") {
      fields.tier = Some(match tier {
        Value::Null => None,
        Value::String(tier) if TIERS.holds(tier) => Some(tier.clone()),
        _ => return Err(unprocessable(fill_marks(&words.tier, &[("tiers", &TIERS.ladder.join(", "))]))),
      });
    }

    Ok(fields)
  }
}

/// A role's key given the tier asked, or its assignment taken away where none is.
async fn set_tier<C: ConnectionTrait + TransactionTrait>(
  db: &C,
  role: &str,
  tier: Option<&str>,
) -> Result<(), Refused> {
  match tier {
    Some(tier) => tiers::assign(db, &key_of(role), tier).await.map_err(Refused::failed),
    None => {
      run(
        db,
        r#"DELETE FROM "model_tier_assignments" WHERE "key" = $1"#,
        vec![key_of(role).into()],
      )
      .await?;

      Ok(())
    }
  }
}

/// `GET /agent-roles`: every role the store holds, and every bundle a role may give.
pub struct AgentRoleList;

impl Route for AgentRoleList {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/agent-roles";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let db = call.store();
    let mut roles = vec![];

    for role in agent_roles::every(db).await? {
      roles.push(view(db, &role).await?);
    }

    let bundles: Vec<Value> = ROLES.bundles.iter().map(|bundle| bundle_view(&bundle.name)).collect();

    Ok(answer(StatusCode::OK, json!({ "roles": roles, "bundles": bundles })))
  }
}

crate::moved!(AgentRoleList);

/// `POST /agent-roles`: a role added, with its key's tier where one is given.
pub struct AgentRoleCreate;

impl Route for AgentRoleCreate {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/agent-roles";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let words = &CONTROL_WORDS.agent_roles;
    let name = body.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();

    if !NAME.is_match(&name) || length(&name) > ROLES.name_most {
      return Err(unprocessable(fill_marks(&words.name, &[("most", &ROLES.name_most.to_string())])));
    }

    let fields = Fields::of(&body)?;
    let Some(label) = fields.label.clone() else {
      return Err(unprocessable(label_refusal()));
    };
    let Some(bundles) = fields.bundles.clone() else {
      return Err(unprocessable(bundles_refusal()));
    };
    let written = call.store().begin().await?;

    if agent_roles::holds(&written, &name).await? {
      return Err(unprocessable(fill_marks(&words.taken, &[("name", &name)])));
    }

    if one(
      &written,
      r#"SELECT 1 FROM "coworker_pool_roles" WHERE "name" = $1"#,
      vec![name.clone().into()],
    )
    .await?
    .is_some()
    {
      return Err(unprocessable(fill_marks(&words.coworker, &[("name", &name)])));
    }

    let now = now_ms();
    let bundles = serde_json::to_string(&bundles).map_err(Refused::failed)?;

    run(
      &written,
      r#"INSERT INTO "agent_roles" ("name", "label", "bundles", "max_concurrent", "prompt", "position", "created_at", "updated_at")
         VALUES ($1, $2, $3, $4, $5, (SELECT COALESCE(MAX("position"), -1) + 1 FROM "agent_roles"), $6, $6)"#,
      vec![
        name.clone().into(),
        label.into(),
        bundles.into(),
        fields.bound.flatten().into(),
        fields.prompt.flatten().into(),
        now.into(),
      ],
    )
    .await?;

    if let Some(tier) = fields.tier.flatten() {
      set_tier(&written, &name, Some(&tier)).await?;
    }

    let role = agent_roles::named(&written, &name).await?.ok_or_else(|| unknown(&name))?;
    let shown = view(&written, &role).await?;

    written.commit().await?;

    Ok(answer(StatusCode::CREATED, shown))
  }
}

crate::moved!(AgentRoleCreate);

/// `PATCH /agent-roles/{roleName}`: a role's label, bundles, bound, prompt or tier changed.
pub struct AgentRoleChange;

impl Route for AgentRoleChange {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/agent-roles/{roleName}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let name = call.param("roleName").unwrap_or_default().to_owned();
    let body = call.body().await?;
    let fields = Fields::of(&body)?;
    let written = call.store().begin().await?;
    let Some(held) = agent_roles::named(&written, &name).await? else {
      return Err(unknown(&name));
    };
    let bundles =
      serde_json::to_string(&fields.bundles.unwrap_or(held.bundles)).map_err(Refused::failed)?;

    run(
      &written,
      r#"UPDATE "agent_roles" SET "label" = $1, "bundles" = $2, "max_concurrent" = $3, "prompt" = $4, "updated_at" = $5
         WHERE "name" = $6"#,
      vec![
        fields.label.unwrap_or(held.label).into(),
        bundles.into(),
        fields.bound.unwrap_or(held.max_concurrent).into(),
        fields.prompt.unwrap_or(held.prompt).into(),
        now_ms().into(),
        name.clone().into(),
      ],
    )
    .await?;

    if let Some(tier) = &fields.tier {
      set_tier(&written, &name, tier.as_deref()).await?;
    }

    let role = agent_roles::named(&written, &name).await?.ok_or_else(|| unknown(&name))?;
    let shown = view(&written, &role).await?;

    written.commit().await?;

    Ok(answer(StatusCode::OK, shown))
  }
}

crate::moved!(AgentRoleChange);

/// `DELETE /agent-roles/{roleName}`: a role no agent holds and no kind is for removed, with its key's
/// assignment.
pub struct AgentRoleDelete;

impl Route for AgentRoleDelete {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/agent-roles/{roleName}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let words = &CONTROL_WORDS.agent_roles;
    let name = call.param("roleName").unwrap_or_default().to_owned();
    let written = call.store().begin().await?;

    if !agent_roles::holds(&written, &name).await? {
      return Err(unknown(&name));
    }

    if let Some((kind, _)) = ROLES.kinds.iter().find(|(_, role)| **role == name) {
      return Err(Refused::new(
        StatusCode::CONFLICT,
        fill_marks(&words.kind_default, &[("name", &name), ("kind", kind)]),
      ));
    }

    let holding = holders(&written, &name).await?;

    if !holding.is_empty() {
      return Err(Refused::new(
        StatusCode::CONFLICT,
        fill_marks(&words.held, &[("name", &name), ("agents", &holding.join(", "))]),
      ));
    }

    run(&written, r#"DELETE FROM "agent_roles" WHERE "name" = $1"#, vec![name.clone().into()]).await?;
    set_tier(&written, &name, None).await?;
    written.commit().await?;

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

crate::moved!(AgentRoleDelete);
