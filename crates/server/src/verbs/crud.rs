//! The one grammar every collection of agency's rows is worked through from the shell (docs/cli.md, 「One grammar
//! for every noun」): `list`, `show <name>`, `add <name>`, `set <name>` and `remove <name>`, each through the
//! server's own routes, so what the console may do to a row an agent may do too, refused where the route refuses
//! it. A change names only the fields it changes, `--set field=value`, and leaves every other field as it stands;
//! a value is read as JSON where it reads as JSON, so `--set enabled=false` is the boolean and `--set
//! model=q38fn-dq8` the text.

use reqwest::Method;
use serde_json::{Map, Value};

use super::{Answer, CONTROL_TOKEN, VerbError, control_call, control_token};

/// One collection the grammar works: the word it is called by, the server's path for it, and the field a new row
/// is named by in the add's body.
#[derive(Clone, Copy, Debug)]
pub struct Collection {
  pub noun: &'static str,
  pub path: &'static str,
  pub name_field: &'static str,
}

/// The role agents, the seats a role's work runs on.
pub const ROLE_AGENTS: Collection = Collection {
  noun: "role-agent",
  path: "/role-agents",
  name_field: "name",
};

/// The repositories' registrations.
pub const REPOSITORIES: Collection = Collection {
  noun: "repository",
  path: "/repositories",
  name_field: "name",
};

/// The supervisors' agency halves: read and changed here; one is added and removed through the receiver, which
/// keeps its room.
pub const SUPERVISORS: Collection = Collection {
  noun: "supervisor",
  path: "/supervisors",
  name_field: "name",
};

/// One step of the grammar.
#[derive(Clone, Debug)]
pub enum Step {
  List,
  Show(String),
  Add(String, Vec<String>),
  Set(String, Vec<String>),
  Remove(String),
}

/// The fields `--set field=value` names, each value read as JSON where it reads as JSON and as text otherwise; a
/// pair with no `=` is refused, naming it.
pub fn fields(pairs: &[String]) -> Result<Map<String, Value>, String> {
  let mut fields = Map::new();

  for pair in pairs {
    let Some((field, value)) = pair.split_once('=') else {
      return Err(format!("`{pair}` names no value: write it as field=value, for example model=q38fn-dq8"));
    };
    let field = field.trim();

    if field.is_empty() {
      return Err(format!("`{pair}` names no field: write it as field=value"));
    }

    let value = serde_json::from_str::<Value>(value).unwrap_or_else(|_| Value::String(value.to_owned()));

    fields.insert(field.to_owned(), value);
  }

  Ok(fields)
}

/// One step on a collection, answered as the lines the shell prints, or the route's refusal with the step that
/// gets past it where the route names one.
pub async fn run(collection: Collection, step: Step) -> Result<Answer, VerbError> {
  let noun = collection.noun;
  let Some(token) = control_token() else {
    return Err(VerbError::Refused(format!("{noun}: {CONTROL_TOKEN} is not set in this process.")));
  };
  let named = match &step {
    Step::List => None,
    Step::Show(name) | Step::Add(name, _) | Step::Set(name, _) | Step::Remove(name) => Some(name.trim()),
  };

  // A name is letters, digits, dots, dashes and underscores, as every row's name is, so it stands in a path as it is.
  if let Some(name) = named
    && (name.is_empty()
      || !name.chars().all(|one| one.is_ascii_alphanumeric() || matches!(one, '.' | '-' | '_')))
  {
    return Err(VerbError::Refused(format!(
      "{noun}: `{name}` is no row's name; `{noun} list` lists the names it holds"
    )));
  }

  let at = |name: &str| format!("{}/{}", collection.path, name.trim());

  let (method, path, body, verb) = match &step {
    Step::List => (Method::GET, collection.path.to_owned(), None, "list"),
    Step::Show(name) => (Method::GET, at(name), None, "show"),
    Step::Add(name, pairs) => {
      let mut body = fields(pairs).map_err(|why| VerbError::Refused(format!("{noun} add: {why}")))?;

      body.insert(collection.name_field.to_owned(), name.trim().into());
      (Method::POST, collection.path.to_owned(), Some(Value::Object(body)), "add")
    }
    Step::Set(name, pairs) => {
      let body = fields(pairs).map_err(|why| VerbError::Refused(format!("{noun} set: {why}")))?;

      if body.is_empty() {
        return Err(VerbError::Refused(format!(
          "{noun} set: name at least one change, as --set field=value; `{noun} show {name}` lists the fields"
        )));
      }

      (Method::PATCH, at(name), Some(Value::Object(body)), "set")
    }
    Step::Remove(name) => (Method::DELETE, at(name), None, "remove"),
  };

  let (status, answer) = control_call(&token, method, &path, body.as_ref())
    .await
    .map_err(|why| VerbError::Refused(format!("{noun} {verb}: agency's server did not answer: {why}")))?;

  answered(collection, &step, status, answer)
}

/// What the route answered, as the lines the shell prints.
pub fn answered(
  collection: Collection,
  step: &Step,
  status: u16,
  answer: Option<Value>,
) -> Result<Answer, VerbError> {
  let noun = collection.noun;

  if !(200..300).contains(&status) {
    let error = answer
      .as_ref()
      .and_then(|answer| answer.get("error"))
      .and_then(Value::as_str)
      .map_or_else(|| format!("agency's server answered {status}"), str::to_owned);
    let next = match (status, step) {
      (404, Step::Show(_) | Step::Set(..) | Step::Remove(_)) => {
        format!(" `{noun} list` lists the names it holds.")
      }
      (409, Step::Add(..)) => {
        format!(" Choose another name, or change the one standing with `{noun} set`.")
      }
      (400 | 422, Step::Add(..) | Step::Set(..)) => {
        format!(" `{noun} show <name>` shows a row's fields as they stand.")
      }
      _ => String::new(),
    };
    // A route that says the step past its refusal itself is said as it is.
    let next = if error.contains('`') { String::new() } else { next };

    return Err(VerbError::Refused(format!("{noun}: {error}{next}")));
  }

  let lines = match step {
    Step::List => listed(answer.as_ref()),
    Step::Show(_) | Step::Add(..) | Step::Set(..) => {
      vec![serde_json::to_string_pretty(&answer.unwrap_or(Value::Null)).unwrap_or_default()]
    }
    Step::Remove(name) => vec![format!("{noun} remove: removed {}.", name.trim())],
  };

  Ok(Answer::from(lines))
}

// A list's rows, one line each: the row's name, and beside it what tells one row from another at a glance.
fn listed(answer: Option<&Value>) -> Vec<String> {
  let rows: Vec<&Value> = match answer {
    Some(Value::Array(rows)) => rows.iter().collect(),
    Some(Value::Object(page)) => page
      .get("items")
      .and_then(Value::as_array)
      .map(|rows| rows.iter().collect())
      .unwrap_or_default(),
    _ => Vec::new(),
  };

  if rows.is_empty() {
    return vec!["(none)".to_owned()];
  }

  rows
    .iter()
    .map(|row| {
      let text = |field: &str| row.get(field).and_then(Value::as_str).filter(|text| !text.is_empty());
      let mut parts = vec![text("name").unwrap_or("?").to_owned()];

      parts.extend(
        ["role", "provider", "model", "effort", "path"]
          .iter()
          .filter_map(|field| text(field))
          .map(str::to_owned),
      );

      if row.get("enabled") == Some(&Value::Bool(false)) {
        parts.push("off".to_owned());
      }

      parts.join(" · ")
    })
    .collect()
}
