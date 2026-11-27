//! A supervisor's accounts on chat services (docs/supervisors.md, 「Its accounts on chat services」; docs/api.md,
//! 「supervisors」): one supervisor to many, each a chat service's key, the account's number there and the name of
//! the saved secret its sign-in is kept in, listed, stood, changed and removed by the supervisor's own routes.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use sea_orm::ConnectionTrait;
use serde_json::{Value, json};
use store::{all, run};

use super::{Call, Refused, Route, answer};

// The most characters a number or a secret's name holds.
const FIELD_MOST: usize = 200;

fn bad(field: &str, error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error).with_field(field)
}

/// The supervisor the path names, refused 404 where none is held.
async fn held<C: ConnectionTrait>(db: &C, name: &str) -> Result<(), Refused> {
  match crate::supervisors::named(db, name).await? {
    Some(_) => Ok(()),
    None => Err(Refused::new(StatusCode::NOT_FOUND, format!("There is no supervisor {name}."))),
  }
}

// A field of one line, 1 to the most characters once trimmed; none where the body does not name it.
fn line_of(body: &Value, field: &str) -> Result<Option<String>, Refused> {
  match body.get(field) {
    None => Ok(None),
    Some(Value::String(text)) => {
      let text = text.trim();

      if text.is_empty() || text.chars().count() > FIELD_MOST || text.contains(['\n', '\r']) {
        Err(bad(field, format!("{field} is one line of 1 to {FIELD_MOST} characters.")))
      } else {
        Ok(Some(text.to_owned()))
      }
    }
    Some(_) => Err(bad(field, format!("{field} is one line of 1 to {FIELD_MOST} characters."))),
  }
}

/// A supervisor's accounts as their rows hold them, by service.
pub async fn accounts_of<C: ConnectionTrait>(db: &C, supervisor: &str) -> Result<Vec<Value>, Refused> {
  let rows = all(
    db,
    r#"SELECT "service", "number", "secret" FROM "supervisor_accounts" WHERE "supervisor" = $1 ORDER BY "service""#,
    vec![supervisor.into()],
  )
  .await?;

  rows
    .iter()
    .map(|row| {
      Ok(json!({
        "service": row.try_get::<String>("", "service").map_err(Refused::failed)?,
        "number": row.try_get::<String>("", "number").map_err(Refused::failed)?,
        "secret": row.try_get::<String>("", "secret").map_err(Refused::failed)?,
      }))
    })
    .collect()
}

async fn account_of<C: ConnectionTrait>(
  db: &C,
  supervisor: &str,
  service: &str,
) -> Result<Option<Value>, Refused> {
  Ok(
    accounts_of(db, supervisor)
      .await?
      .into_iter()
      .find(|account| account["service"] == service),
  )
}

/// `GET /supervisors/{supervisorName}/accounts`: the supervisor's accounts, by service.
pub struct SupervisorAccountsRead;

impl Route for SupervisorAccountsRead {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/supervisors/{supervisorName}/accounts";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let name = call.param("supervisorName").unwrap_or_default().to_owned();

    held(call.store(), &name).await?;

    Ok(answer(StatusCode::OK, json!({ "items": accounts_of(call.store(), &name).await? })))
  }
}

crate::moved!(SupervisorAccountsRead);

/// `PUT /supervisors/{supervisorName}/accounts/{service}`: the account of that service stood whole.
pub struct SupervisorAccountStand;

impl Route for SupervisorAccountStand {
  const METHOD: Method = Method::PUT;
  const PATH: &'static str = "/supervisors/{supervisorName}/accounts/{service}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let name = call.param("supervisorName").unwrap_or_default().to_owned();
    let service = call.param("service").unwrap_or_default().to_owned();
    let body = call.body().await?;

    held(call.store(), &name).await?;

    let number = line_of(&body, "number")?.ok_or_else(|| bad("number", "number is required."))?;
    let secret = line_of(&body, "secret")?.ok_or_else(|| bad("secret", "secret is required."))?;

    run(
      call.store(),
      r#"INSERT INTO "supervisor_accounts" ("supervisor", "service", "number", "secret") VALUES ($1, $2, $3, $4)
         ON CONFLICT ("supervisor", "service") DO UPDATE SET "number" = EXCLUDED."number", "secret" = EXCLUDED."secret""#,
      vec![name.clone().into(), service.clone().into(), number.into(), secret.into()],
    )
    .await?;

    Ok(answer(
      StatusCode::OK,
      account_of(call.store(), &name, &service).await?.unwrap_or(Value::Null),
    ))
  }
}

crate::moved!(SupervisorAccountStand);

/// `PATCH /supervisors/{supervisorName}/accounts/{service}`: the fields the body names changed.
pub struct SupervisorAccountChange;

impl Route for SupervisorAccountChange {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/supervisors/{supervisorName}/accounts/{service}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let name = call.param("supervisorName").unwrap_or_default().to_owned();
    let service = call.param("service").unwrap_or_default().to_owned();
    let body = call.body().await?;

    held(call.store(), &name).await?;

    let Some(standing) = account_of(call.store(), &name, &service).await? else {
      return Err(Refused::new(StatusCode::NOT_FOUND, format!("{name} holds no account on {service}.")));
    };
    let number = line_of(&body, "number")?
      .unwrap_or_else(|| standing["number"].as_str().unwrap_or_default().to_owned());
    let secret = line_of(&body, "secret")?
      .unwrap_or_else(|| standing["secret"].as_str().unwrap_or_default().to_owned());

    run(
      call.store(),
      r#"UPDATE "supervisor_accounts" SET "number" = $3, "secret" = $4 WHERE "supervisor" = $1 AND "service" = $2"#,
      vec![name.clone().into(), service.clone().into(), number.into(), secret.into()],
    )
    .await?;

    Ok(answer(
      StatusCode::OK,
      account_of(call.store(), &name, &service).await?.unwrap_or(Value::Null),
    ))
  }
}

crate::moved!(SupervisorAccountChange);

/// `DELETE /supervisors/{supervisorName}/accounts/{service}`: the account of that service removed.
pub struct SupervisorAccountRemove;

impl Route for SupervisorAccountRemove {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/supervisors/{supervisorName}/accounts/{service}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let name = call.param("supervisorName").unwrap_or_default().to_owned();
    let service = call.param("service").unwrap_or_default().to_owned();

    held(call.store(), &name).await?;

    let removed = run(
      call.store(),
      r#"DELETE FROM "supervisor_accounts" WHERE "supervisor" = $1 AND "service" = $2"#,
      vec![name.clone().into(), service.clone().into()],
    )
    .await?;

    if removed == 0 {
      return Err(Refused::new(StatusCode::NOT_FOUND, format!("{name} holds no account on {service}.")));
    }

    Ok(answer(StatusCode::OK, json!({ "supervisor": name, "service": service })))
  }
}

crate::moved!(SupervisorAccountRemove);
