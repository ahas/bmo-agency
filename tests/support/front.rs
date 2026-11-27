//! The front's writing of the agency's organisation, as a test stands it in (docs/api.md, 「What the front writes」):
//! every setting the agency lists resolved for the people and units a test names, asked of the account service the
//! test stands up as the console asks its own, and written whole into the agency's subject settings; and the units each
//! account heads written whole into the unit reach table.

use agency_server::chain::{self, Held};
use agency_server::unit_reach::{self, Reached};
use sea_orm::DatabaseConnection;
use serde_json::Value;

/// Every setting the agency lists, resolved for each person and unit at the account service standing at `address`
/// behind `token`, written whole into the subject settings; a resolution the service does not answer writes nothing.
pub async fn resolve(db: &DatabaseConnection, address: &str, token: &str, people: &[i64], units: &[i64]) {
  resolve_keys(db, address, token, people, units, |_| true).await;
}

/// The front's writing narrowed to the settings `asked` keeps: a test whose account service answers only some.
pub async fn resolve_keys(
  db: &DatabaseConnection,
  address: &str,
  token: &str,
  people: &[i64],
  units: &[i64],
  asked: impl Fn(&str) -> bool,
) {
  let client = reqwest::Client::new();
  // The kinds the agency reads beyond its own, an app's among them, as the account service lists them.
  let listed: Vec<String> =
    match client.get(format!("{address}/chain/kinds")).bearer_auth(token).send().await {
      Ok(answer) if answer.status().is_success() => answer
        .json::<Value>()
        .await
        .ok()
        .and_then(|body| body["kinds"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|kind| kind["key"].as_str())
        .filter(|key| key.starts_with(agency_server::apps::KIND_HEAD))
        .map(str::to_owned)
        .collect(),
      _ => Vec::new(),
    };
  let keys: Vec<String> = chain::kinds()
    .into_iter()
    .map(|kind| kind.key)
    .chain(chain::read_beyond())
    .chain(listed)
    .filter(|key| asked(key))
    .collect();
  let subjects = people
    .iter()
    .map(|person| ("person", *person))
    .chain(units.iter().map(|unit| ("unit", *unit)));
  let mut rows = Vec::new();

  for (kind, id) in subjects {
    for setting in &keys {
      let answer = client
        .get(format!("{address}/chain/kinds/{setting}/resolve"))
        .query(&[(kind, id.to_string())])
        .bearer_auth(token)
        .send()
        .await;
      let Ok(answer) = answer else { continue };

      if !answer.status().is_success() {
        continue;
      }

      let Ok(value) = answer.json::<Value>().await else { continue };

      rows.push(Held {
        subject: format!("{kind}:{id}"),
        key: setting.clone(),
        value,
      });
    }
  }

  chain::replace_held(db, &rows).await.unwrap();
}

/// The units each account heads, written whole into the unit reach table.
pub async fn heads(db: &DatabaseConnection, pairs: &[(i64, i64)]) {
  let rows: Vec<Reached> = pairs
    .iter()
    .map(|(account, unit)| Reached {
      account: *account,
      unit: *unit,
      write: true,
    })
    .collect();

  unit_reach::replace(db, &rows).await.unwrap();
}

/// The front's writing for every person and unit a test of this harness names: people and units 1 to 60.
pub async fn resolve_all(db: &DatabaseConnection, address: &str, token: &str) {
  let people: Vec<i64> = (1..=60).collect();
  let units: Vec<i64> = (1..=60).collect();

  resolve(db, address, token, &people, &units).await;
}
