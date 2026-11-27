//! Whose rows a call changes on the agent management routes (docs/api.md, 「Who a caller acts for」): the agency
//! judges no person's permission — the service in front of it does, before a call is made — and keeps only the
//! line between owners. A caller carrying no `Owner-Key` is the deployment's own and keeps every row; one
//! carrying `Owner-Role: administrator` is the platform's and keeps every row too. A member, named by the key the
//! front hands on, changes only the rows filed under their own account and those of the units the unit reach
//! table says they head (`crate::unit_reach`), files what they add under their own account, and changes no
//! role, the roles being the platform's presets. A supervisor's or another holder's work stands only on a
//! repository of its own side (「works_on」).

use axum::http::{Method, StatusCode};

use crate::control::{Call, Refused};
use crate::places::Ground;

/// One part of agent management: a kind of route, and the console section of the tab that holds it (the web's
/// `docs/accounts.md`, 「Console sections」). A role's agents stand under the roles section with the roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
  Supervisors,
  Roles,
  RoleAgents,
  Repositories,
  Models,
  ModelFiles,
}

impl Part {
  const ALL: [Self; 6] = [
    Self::Supervisors,
    Self::Roles,
    Self::RoleAgents,
    Self::Repositories,
    Self::Models,
    Self::ModelFiles,
  ];

  /// The path the part's routes stand under.
  fn root(self) -> &'static str {
    match self {
      Part::Supervisors => "/supervisors",
      Self::Roles => "/roles",
      Part::RoleAgents => "/role-agents",
      Self::Repositories => "/repositories",
      Self::Models => "/models",
      Self::ModelFiles => "/model-catalogue",
    }
  }

  /// The path parameter that names the row a route of this part changes, none for a part whose rows are judged
  /// whatever the path names.
  fn row_param(self) -> Option<&'static str> {
    match self {
      Part::Supervisors => Some("supervisorName"),
      Self::Repositories => Some("repositoryName"),
      Part::RoleAgents => Some("agentName"),
      Self::Models => Some("modelKey"),
      Self::Roles | Self::ModelFiles => None,
    }
  }

  /// Whose the row named is: `Unnamed` for a route naming none — an add, which its route files — or a row the
  /// store does not hold, which its route answers; the account it is filed under, none for the platform's; and
  /// `Unjudged` for a part whose rows carry their owner on routes of their own.
  async fn row(self, ground: &Ground, name: Option<String>) -> Result<Row, Refused> {
    let db = &ground.db;

    Ok(match (self, name) {
      (Self::Roles, _) => Row::Platform,
      (Self::Models | Self::ModelFiles, _) => Row::Unjudged,
      (_, None) => Row::Unnamed,
      (Self::Supervisors, Some(name)) => crate::supervisors::named(db, &name)
        .await?
        .map_or(Row::Unnamed, |held| Row::Owned("supervisor", name, held.owner_account, held.owner_unit)),
      (Self::Repositories, Some(name)) => crate::repositories::named(db, &name)
        .await?
        .map_or(Row::Unnamed, |held| Row::Owned("repository", name, held.owner_account, held.owner_unit)),
      // A role agent is a seat of its account or of a unit.
      (Self::RoleAgents, Some(name)) => crate::role_agents::named(db, &name)
        .await?
        .map_or(Row::Unnamed, |held| Row::Owned("role agent", name, held.owner_account, held.owner_unit)),
    })
  }

  /// The part a path stands under, none for a route of another kind.
  fn of(path: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|part| {
      let root = part.root();

      path == root || path.starts_with(root) && path.as_bytes().get(root.len()) == Some(&b'/')
    })
  }
}

/// Whose the row a managed route changes is (「Part::row」).
enum Row {
  /// A row of the kind named, by its name, filed under an account or, with neither owner, the platform's.
  Owned(&'static str, String, Option<i64>, Option<i64>),
  /// A row no account owns: a role, the platform's preset.
  Platform,
  /// No row the store holds, or none the route names.
  Unnamed,
  /// A row whose owner its own route judges: a registered model's or a model file's.
  Unjudged,
}

/// A member's change of a managed route, whose row is judged before the route answers.
pub struct Owning {
  acting: Acting,
  part: Part,
  name: Option<String>,
  ground: std::sync::Arc<Ground>,
}

/// The ownership a call's change needs judged (docs/api.md, 「Who a caller acts for」), none for a read, a route of
/// another kind or the platform's own call; or the refusal of a member's key that names no user id.
pub fn owning(call: &Call, method: &Method, path: &str) -> Result<Option<Owning>, Refused> {
  let Some(part) = Part::of(path) else {
    return Ok(None);
  };

  if matches!(method, &Method::GET | &Method::HEAD) {
    return Ok(None);
  }

  let acting = Acting::of(call)?;

  if acting == Acting::Platform {
    return Ok(None);
  }

  Ok(Some(Owning {
    acting,
    part,
    name: part.row_param().and_then(|param| call.param(param)).map(str::to_owned),
    ground: call.shared_ground(),
  }))
}

/// Whether a member's change reaches a row they may change: a row of another account or of the platform is
/// refused, and so is every change of a role; a route naming no row the store holds goes on to answer.
pub async fn judge(owning: Owning) -> Result<(), Refused> {
  if owning.part == Part::Repositories {
    crate::organisation::repositories::may_manage(&owning.ground, owning.acting).await?;

    if let Some(name) = &owning.name
      && let Some(held) = crate::repositories::named(&owning.ground.db, name).await?
      && let Some(unit) = held.owner_unit
    {
      return crate::organisation::repositories::heads(&owning.ground, owning.acting, unit, name).await;
    }
  }

  match owning.part.row(&owning.ground, owning.name).await? {
    // A row a unit owns is its heads' alone, the chain's answer of who writes the unit's link; the same rule a
    // unit's repository stands on.
    Row::Owned(kind, name, _, Some(unit)) => {
      if crate::organisation::resources::heads(&owning.ground, owning.acting, unit).await? {
        Ok(())
      } else {
        Err(crate::organisation::resources::refused(&format!("the {kind} {name}")))
      }
    }
    Row::Owned(kind, name, owner, None) => {
      owning.acting.may_change(owner, &format!("the {kind} {name}"))
    }
    Row::Platform => Err(forbidden(
      "The signed-in person may not change a role: the roles are the platform's presets, the administrator's alone"
        .to_owned(),
    )),
    Row::Unnamed | Row::Unjudged => Ok(()),
  }
}

/// A member's refusal, saying what was asked and why.
fn forbidden(error: String) -> Refused {
  Refused::new(StatusCode::FORBIDDEN, error).with_code("forbidden")
}

/// Whether work held by an account may stand on a repository (docs/api.md, 「assignments and jobs」): a repository
/// filed under an account takes the work of that account alone, and one filed under none, the platform's, the work
/// of the platform's side alone — a holder of no account or of the administrator's own. Refused 403
/// `foreign-repository` naming the repository otherwise; a repository the store does not hold is its route's to
/// answer.
pub async fn works_on(ground: &Ground, account: Option<i64>, repository: &str) -> Result<(), Refused> {
  let Some(held) = crate::repositories::named(&ground.db, repository).await? else {
    return Ok(());
  };

  if let Some(unit) = held.owner_unit {
    return crate::organisation::repositories::works_on_unit(ground, account, unit, repository).await;
  }

  if same_side(&ground.db, account, held.owner_account).await? {
    return Ok(());
  }

  let whose = match held.owner_account {
    Some(_) => "another account's",
    None => "the platform's",
  };
  let holder = match account {
    Some(account) => format!("account {account}'s"),
    None => "the platform's".to_owned(),
  };

  Err(
    Refused::new(
      StatusCode::FORBIDDEN,
      format!("The repository {repository} is {whose}: {holder} work does not stand on it"),
    )
    .with_code("foreign-repository"),
  )
}

/// Whether a holder's account and a repository's owner stand on one side: the same account, or both the
/// platform's, where the administrator's own account stands with the platform.
async fn same_side(
  db: &sea_orm::DatabaseConnection,
  account: Option<i64>,
  owner: Option<i64>,
) -> Result<bool, Refused> {
  if account == owner {
    return Ok(true);
  }

  let platform = |held: Option<i64>| async move {
    match held {
      None => Ok::<_, Refused>(true),
      Some(id) => Ok(crate::account_isolation::owners_own(db, &id.to_string()).await?),
    }
  };

  Ok(platform(account).await? && platform(owner).await?)
}

/// Whom a call on a person's own supply acts for (docs/api.md, 「Who a caller acts for」): a person the console
/// names as a member, on their own accounts and keys alone, or the platform — the administrator, or the
/// deployment's own caller carrying no key — on the platform's accounts and of each person's supply only the
/// roads it serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acting {
  Person(i64),
  Platform,
}

impl Acting {
  /// Whom the call acts for, or the refusal of a member's key that names no user id.
  pub fn of(call: &Call) -> Result<Self, Refused> {
    let Some(key) = call.header("owner-key").map(|key| key.trim().to_owned()).filter(|key| !key.is_empty())
    else {
      return Ok(Self::Platform);
    };

    if call
      .header("owner-role")
      .is_some_and(|role| role.trim().eq_ignore_ascii_case("administrator"))
    {
      return Ok(Self::Platform);
    }

    key.parse::<i64>().map(Self::Person).map_err(|_| {
      Refused::new(StatusCode::FORBIDDEN, "The signed-in person is named by no user id agency reads")
        .with_code("forbidden")
    })
  }

  /// Whether the caller reaches the rows filed under an account: the platform every account's, a person their
  /// own alone. The one place a person's reach over the rows of others would be granted.
  fn reaches(self, account: i64) -> bool {
    match self {
      Self::Platform => true,
      Self::Person(user) => user == account,
    }
  }

  /// Whether the caller may change a row filed under `owner`, none being the platform's, or the refusal naming
  /// `what` it asked: the platform changes every row, a person the rows of the accounts they reach alone.
  pub fn may_change(self, owner: Option<i64>, what: &str) -> Result<(), Refused> {
    match (self, owner) {
      (Self::Platform, _) => Ok(()),
      (_, Some(owner)) if self.reaches(owner) => Ok(()),
      (_, Some(_)) => Err(forbidden(format!(
        "The signed-in person may not change {what}: it is filed under another account"
      ))),
      (_, None) => Err(forbidden(format!(
        "The signed-in person may not change {what}: it is the platform's, the administrator's alone"
      ))),
    }
  }

  /// The account a row the caller adds is filed under, given the one its body names, none where it names none:
  /// the platform files it where the body says; a person files it under their own account, and a body naming an
  /// account they do not reach is refused.
  pub fn files_under(self, named: Option<i64>, what: &str) -> Result<Option<i64>, Refused> {
    match (self, named) {
      (Self::Platform, named) => Ok(named),
      (Self::Person(_), Some(named)) if self.reaches(named) => Ok(Some(named)),
      (Self::Person(_), Some(_)) => Err(forbidden(format!(
        "The signed-in person may not file {what} under another account: what they add is filed under their own"
      ))),
      (Self::Person(user), None) => Ok(Some(user)),
    }
  }

  /// The person the call acts for, none for the platform.
  pub fn person(self) -> Option<i64> {
    match self {
      Self::Person(user) => Some(user),
      Self::Platform => None,
    }
  }
}
