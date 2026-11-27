//! The tables the service in front of the agency writes whole (docs/api.md, 「What the front writes」): the units each
//! account reaches and the values of the settings the agency declares, resolved per person and per unit; and the
//! declarations of those settings, which the front reads to know what to resolve. The agency keeps no organisation
//! and no delegation chain of its own: it answers these to the deployment's own caller alone and reads them for
//! every judgement a unit's reach or a chained setting makes.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer};
use crate::chain::{self, Held};
use crate::unit_reach::{self, Reached};

/// The refusal of a call made in a person's name: the tables are the front's to write, never a person's.
fn persons_call(call: &Call) -> Result<(), Refused> {
  if call.header("owner-key").is_some_and(|key| !key.trim().is_empty()) {
    return Err(
      Refused::new(StatusCode::FORBIDDEN, "The front's tables are read and written by the deployment's own call alone")
        .with_code("forbidden"),
    );
  }

  Ok(())
}

fn bad(why: impl std::fmt::Display) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, format!("The body is no list of rows this table holds: {why}"))
    .with_code("bad-request")
}

#[derive(Deserialize)]
struct Rows<T> {
  items: Vec<T>,
}

/// `GET /unit-reach`: every account's reach of every unit, by account and unit.
pub struct UnitReachRead;

impl Route for UnitReachRead {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/unit-reach";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    persons_call(&call)?;

    let items = unit_reach::every(&call.ground().db).await?;

    Ok(answer(StatusCode::OK, json!({ "items": items })))
  }
}

crate::moved!(UnitReachRead);

/// `PUT /unit-reach`: the table replaced whole by the rows the body lists, answered as it now stands.
pub struct UnitReachWrite;

impl Route for UnitReachWrite {
  const METHOD: Method = Method::PUT;
  const PATH: &'static str = "/unit-reach";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    persons_call(&call)?;

    let body = call.body().await?;
    let rows: Rows<Reached> = serde_json::from_value(body).map_err(bad)?;
    let db = &call.ground().db;

    unit_reach::replace(db, &rows.items).await?;

    Ok(answer(StatusCode::OK, json!({ "items": unit_reach::every(db).await? })))
  }
}

crate::moved!(UnitReachWrite);

/// `GET /subject-settings`: every subject's value of every setting the agency holds.
pub struct SubjectSettingsRead;

impl Route for SubjectSettingsRead {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/subject-settings";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    persons_call(&call)?;

    let items = chain::every_held(&call.ground().db).await.map_err(Refused::failed)?;

    Ok(answer(StatusCode::OK, json!({ "items": items })))
  }
}

crate::moved!(SubjectSettingsRead);

/// `PUT /subject-settings`: the values replaced whole by the rows the body lists, answered as they now stand. A
/// subject is `person:<id>` or `unit:<id>`.
pub struct SubjectSettingsWrite;

impl Route for SubjectSettingsWrite {
  const METHOD: Method = Method::PUT;
  const PATH: &'static str = "/subject-settings";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    persons_call(&call)?;

    let body = call.body().await?;
    let rows: Rows<Held> = serde_json::from_value(body).map_err(bad)?;

    if let Some(row) = rows.items.iter().find(|row| !subject_named(&row.subject)) {
      return Err(bad(format!("{} names no person:<id> or unit:<id>", row.subject)));
    }

    let db = &call.ground().db;

    chain::replace_held(db, &rows.items).await.map_err(Refused::failed)?;

    Ok(answer(StatusCode::OK, json!({ "items": chain::every_held(db).await.map_err(Refused::failed)? })))
  }
}

crate::moved!(SubjectSettingsWrite);

// Whether a subject is written as the agency reads one.
fn subject_named(subject: &str) -> bool {
  subject
    .split_once(':')
    .is_some_and(|(kind, id)| matches!(kind, "person" | "unit") && id.parse::<i64>().is_ok())
}

/// `GET /setting-kinds`: the settings the agency declares, each as the front declares it on its own chain: its key,
/// what it is for, its shape, its choices, its mode, the levels it stands on and its default.
pub struct SettingKinds;

impl Route for SettingKinds {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/setting-kinds";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    persons_call(&call)?;

    let kinds: Vec<Value> = chain::kinds().iter().map(chain::declaration).collect();

    Ok(answer(StatusCode::OK, json!({ "kinds": kinds })))
  }
}

crate::moved!(SettingKinds);
