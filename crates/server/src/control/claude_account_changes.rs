//! What the owner changes about the Claude accounts from the fallback screen (docs/cache-fallback.md,
//! 「The Claude Code road is a list of accounts」): an account added, renamed, moved to another
//! credential, turned on or off, put in order, removed, and chosen to run on now. What is kept is where
//! a credential is reached, never a credential.

use std::path::Path;
use std::sync::LazyLock;

use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use regex::Regex;
use sea_orm::{DatabaseConnection, TransactionTrait};
use serde_json::Value;

use super::claude_accounts::{Account, CHOSEN_KEY, IN_USE_KEY, accounts, chosen_account};
use super::{Refused, set_setting, setting};
use crate::now_ms;
use crate::text::length;
use store::{one, run};

// What an id may be: it stands in a work key's row, a CLI flag and a session record's key, so it is
// the small alphabet all three take without quoting.
static ID: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9._-]{1,31}$").expect("the account id pattern compiles"));

const NAME_MAX: usize = 40;

/// How a run reaches an account's login, and the absolute path it is reached by.
#[derive(Clone, Debug)]
pub struct Credential {
  pub kind: String,
  pub path: String,
}

fn no_such(id: &str) -> String {
  fill_marks(&CONTROL_WORDS.accounts.unknown, &[("id", id)])
}

/// The id minted for a new account from the name the owner gave it: never the owner's to type, since every
/// record keys on it, and made even for a name with nothing usable in it.
pub fn minted_id(name: &str, taken: &[&str]) -> Result<String, String> {
  let mut slug = String::new();

  for letter in name.to_lowercase().chars() {
    if letter.is_ascii_lowercase() || letter.is_ascii_digit() || matches!(letter, '.' | '_' | '-') {
      slug.push(letter);
    } else if !slug.ends_with('\u{0}') {
      slug.push('\u{0}');
    }
  }

  let slug = slug.replace('\u{0}', "-");
  let slug =
    slug.trim_start_matches(|letter: char| !letter.is_ascii_lowercase() && !letter.is_ascii_digit());
  let slug = slug.trim_end_matches(['-', '.', '_']);
  let slug: String = slug.chars().take(32).collect();
  let stem = if ID.is_match(&slug) { slug } else { "account".to_owned() };

  if !taken.contains(&stem.as_str()) {
    return Ok(stem);
  }

  let head: String = stem.chars().take(28).collect();

  (2..1000)
    .map(|next| format!("{head}-{next}"))
    .find(|candidate| !taken.contains(&candidate.as_str()))
    .ok_or_else(|| CONTROL_WORDS.accounts.no_id.clone())
}

/// Why a name cannot be an account's beside the names others hold, or nothing.
pub fn name_fault(name: &str, taken: &[&str]) -> Option<String> {
  let given = name.trim();

  if given.is_empty() {
    return Some(CONTROL_WORDS.accounts.name_empty.clone());
  }

  if length(given) > NAME_MAX {
    return Some(fill_marks(&CONTROL_WORDS.accounts.name_long, &[("most", &NAME_MAX.to_string())]));
  }

  taken
    .contains(&given)
    .then(|| fill_marks(&CONTROL_WORDS.accounts.name_taken, &[("name", given)]))
}

/// Why a credential cannot be reached, or nothing: a directory account needs a directory that is
/// there, a token account the file its token sits in. Neither is opened.
pub fn credential_fault(credential: &Credential) -> Option<String> {
  let path = credential.path.trim();

  if path.is_empty() {
    return Some(CONTROL_WORDS.accounts.path_empty.clone());
  }

  let path = Path::new(path);

  if !path.is_absolute() {
    return Some(CONTROL_WORDS.accounts.path_relative.clone());
  }

  if !path.exists() {
    return Some(CONTROL_WORDS.accounts.path_missing.clone());
  }

  match (credential.kind == "directory", path.is_dir()) {
    (true, false) => Some(CONTROL_WORDS.accounts.path_not_directory.clone()),
    (false, true) => Some(CONTROL_WORDS.accounts.path_not_file.clone()),
    _ => None,
  }
}

/// One account by its id, or by the name the owner calls it.
pub async fn account_named(db: &DatabaseConnection, named: &str) -> Result<Option<Account>, Refused> {
  let held = accounts(db).await?;
  let by_id = held.iter().position(|one| one.id == named);
  let by_name = || held.iter().position(|one| one.name == named.trim());

  Ok(by_id.or_else(by_name).map(|at| held[at].clone()))
}

/// An account the owner named added at the end of the order, minted its own id, or the refusal. The
/// name and the path are stored with their edges trimmed; a relative path is refused, never resolved.
pub async fn add(db: &DatabaseConnection, name: &str, credential: &Credential) -> Result<(), Refused> {
  let held = accounts(db).await?;
  let ids: Vec<&str> = held.iter().map(|one| one.id.as_str()).collect();
  let names: Vec<&str> = held.iter().map(|one| one.name.as_str()).collect();
  let id = minted_id(name, &ids).map_err(Refused::failed)?;
  let asked = Credential {
    kind: credential.kind.clone(),
    path: credential.path.trim().to_owned(),
  };

  if let Some(fault) = name_fault(name, &names).or_else(|| credential_fault(&asked)) {
    return Err(Refused::failed(fault));
  }

  let written = db.begin().await?;
  let last = one(&written, "SELECT MAX(place) AS place FROM claude_accounts", vec![])
    .await?
    .map(|row| row.try_get::<Option<i64>>("", "place"))
    .transpose()?
    .flatten();

  run(
    &written,
    "INSERT INTO claude_accounts (id, name, place, enabled, credential_kind, credential_path, added_at)
     VALUES ($1, $2, $3, 1, $4, $5, $6)",
    vec![
      id.into(),
      name.trim().into(),
      (last.unwrap_or(0) + 1).into(),
      asked.kind.into(),
      asked.path.into(),
      now_ms().div_euclid(1000).into(),
    ],
  )
  .await?;
  written.commit().await?;

  Ok(())
}

/// What the owner changed about one account by its id — its name, its credential, its switch — or the
/// refusal: only what was named is judged, and the last account that is on stays on.
pub async fn update(
  db: &DatabaseConnection,
  id: &str,
  name: Option<&str>,
  credential: Option<&Credential>,
  enabled: Option<bool>,
) -> Result<(), Refused> {
  let held = accounts(db).await?;

  if !held.iter().any(|one| one.id == id) {
    return Err(Refused::failed(no_such(id)));
  }

  let others: Vec<&Account> = held.iter().filter(|one| one.id != id).collect();
  let names: Vec<&str> = others.iter().map(|one| one.name.as_str()).collect();
  let credential = credential.map(|given| Credential {
    kind: given.kind.clone(),
    path: given.path.trim().to_owned(),
  });
  let fault = name
    .and_then(|name| name_fault(name, &names))
    .or_else(|| credential.as_ref().and_then(credential_fault))
    .or_else(|| {
      (enabled == Some(false) && !others.iter().any(|one| one.enabled))
        .then(|| CONTROL_WORDS.accounts.last_off.as_str().into())
    });

  if let Some(fault) = fault {
    return Err(Refused::failed(fault));
  }

  let mut sets = Vec::new();
  let mut values: Vec<sea_orm::Value> = Vec::new();
  let mut set = |column: &str, value: sea_orm::Value| {
    values.push(value);
    sets.push(format!("{column} = ${}", values.len()));
  };

  if let Some(name) = name {
    set("name", name.trim().into());
  }

  if let Some(enabled) = enabled {
    set("enabled", i64::from(enabled).into());
  }

  if let Some(credential) = credential {
    set("credential_kind", credential.kind.into());
    set("credential_path", credential.path.into());
  }

  if !sets.is_empty() {
    values.push(id.into());
    run(
      db,
      &format!("UPDATE claude_accounts SET {} WHERE id = ${}", sets.join(", "), values.len()),
      values,
    )
    .await?;
  }

  let_choice_go_if_off(db).await
}

/// One account removed by its id, or the refusal: the last one stays, since every road needs an
/// account to name.
pub async fn remove(db: &DatabaseConnection, id: &str) -> Result<(), Refused> {
  let held = accounts(db).await?;

  if !held.iter().any(|one| one.id == id) {
    return Err(Refused::failed(no_such(id)));
  }

  if held.len() == 1 {
    return Err(Refused::failed(CONTROL_WORDS.accounts.last_gone.as_str()));
  }

  run(db, "DELETE FROM claude_accounts WHERE id = $1", vec![id.into()]).await?;

  let_choice_go_if_off(db).await
}

/// An order over exactly the accounts held, as the entries it saves: every one once, its id alone —
/// keeping the switch it has — or with its on/off for all of them, never the two mixed.
pub fn order_to_save(value: &Value, held: &[Account]) -> Option<Vec<(String, bool)>> {
  let entries = value.as_array().filter(|entries| entries.len() == held.len())?;
  let read: Vec<(String, bool)> = if entries.iter().all(Value::is_string) {
    entries
      .iter()
      .map(|one| {
        let id = one.as_str()?;

        Some((id.to_owned(), held.iter().find(|held| held.id == id)?.enabled))
      })
      .collect::<Option<_>>()?
  } else {
    entries
      .iter()
      .map(|one| Some((one.get("id")?.as_str()?.to_owned(), one.get("enabled")?.as_bool()?)))
      .collect::<Option<_>>()?
  };
  let unique = read
    .iter()
    .enumerate()
    .all(|(at, (id, _))| read[..at].iter().all(|(before, _)| before != id));

  (unique && read.iter().all(|(id, _)| held.iter().any(|one| &one.id == id))).then_some(read)
}

/// The order the owner put the accounts in, places and switches written as one, or the refusal of
/// one leaving every account off.
pub async fn set_order(db: &DatabaseConnection, order: &[(String, bool)]) -> Result<(), Refused> {
  if !order.iter().any(|(_, enabled)| *enabled) {
    return Err(Refused::failed(CONTROL_WORDS.accounts.last_off.as_str()));
  }

  let written = db.begin().await?;

  for (place, (id, enabled)) in order.iter().enumerate() {
    run(
      &written,
      "UPDATE claude_accounts SET place = $1, enabled = $2 WHERE id = $3",
      vec![(place as i64 + 1).into(), i64::from(*enabled).into(), id.clone().into()],
    )
    .await?;
  }

  written.commit().await?;

  let_choice_go_if_off(db).await
}

/// One account chosen to run on now, which moves the login in use with it; or the choice let go,
/// which hands the road back to the order.
pub async fn choose(db: &DatabaseConnection, id: Option<&str>) -> Result<(), Refused> {
  let Some(id) = id else {
    for key in [IN_USE_KEY, CHOSEN_KEY] {
      run(db, r#"DELETE FROM "settings" WHERE "key" = $1"#, vec![key.into()]).await?;
    }

    return Ok(());
  };

  match accounts(db).await?.iter().find(|one| one.id == id) {
    None => return Err(Refused::failed(no_such(id))),
    Some(account) if !account.enabled => {
      return Err(Refused::failed(CONTROL_WORDS.accounts.chosen_off.as_str()));
    }
    Some(_) => {}
  }

  set_setting(db, IN_USE_KEY, id).await?;
  set_setting(db, CHOSEN_KEY, id).await
}

// A choice naming an account that is no longer on is let go rather than left to wake again.
async fn let_choice_go_if_off(db: &DatabaseConnection) -> Result<(), Refused> {
  if setting(db, CHOSEN_KEY).await?.is_some() && chosen_account(db).await?.is_none() {
    run(db, r#"DELETE FROM "settings" WHERE "key" = $1"#, vec![CHOSEN_KEY.into()]).await?;
  }

  Ok(())
}
