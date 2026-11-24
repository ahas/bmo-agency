//! The apps a person's sessions reach (docs/tools.md, 「The apps a person reaches」): each app's two settings on the
//! account service's delegation chain, `app.<key>.offered` and `app.<key>.on`, resolved for the person a session
//! acts for, and the apps that are not both written into the file the session's record names as its
//! `withheldAppsFile`, which the tool host reads at every listing, description and call; and beside them what the
//! person's personal tools may reach, the chain's `personal-apps.network` and `personal-apps.secrets` for the person
//! (docs/person-nodes.md, 「The network and secrets of a call」); and with them the keys of the account's MCP servers
//! not attached to the record's holder (docs/person-nodes.md, 「Attaching skills and MCP servers」), for the
//! administrator's account alone where it is theirs. Resolved as a record is issued and again as each turn of a
//! resident program begins.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tool_call::room_gate::AccountService;

use crate::Refusal;
use crate::person_nodes::{Caller, acting_for};
use crate::places::Ground;

/// The file a place keeps its program's withheld apps in.
pub const PLACE_FILE: &str = "withheld-apps.json";

/// The folder under the data root a run naming no place keeps its file in.
const RUN_FOLDER: &str = "agency/withheld-apps";

/// The prefix and the suffix of the kind key of an app's offered setting.
const KIND_HEAD: &str = "app.";
const OFFERED_TAIL: &str = ".offered";

/// What a withheld-apps file holds: the keys of the apps that do not reach the person, the account they were
/// resolved for, which a turn resolves them again for, and what the person's personal tools may reach.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Withheld {
  pub apps: BTreeSet<String>,
  pub account: String,
  #[serde(default)]
  pub personal: Personal,
  /// The holder the record is issued for, whose attached MCP servers it reaches.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub holder: Option<crate::attachments::Holder>,
  /// The line the record's runs go out on.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub line: Option<String>,
}

/// Whom a record is issued for: its holder and its line, which decide the MCP servers it reaches.
#[derive(Clone, Debug, Default)]
pub struct Issued {
  pub holder: Option<crate::attachments::Holder>,
  pub line: Option<String>,
}

/// What a person's personal tools may reach beyond their own folders, as the chain allows it for them: the network
/// at the addresses a tool's manifest names, and the secrets it names. Neither where the chain cannot be asked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Personal {
  pub network: bool,
  pub secrets: bool,
}

/// What `person`'s personal tools may reach: the two flags resolved for them, each off where it does not resolve.
pub async fn personal(service: &AccountService, person: &str) -> Personal {
  let Ok(person) = person.parse::<i64>() else {
    return Personal::default();
  };
  let settings = &policy::organisation::ORGANISATION.personal_apps;
  let allows = |key: &'static str| async move {
    crate::chain::resolve(service, key, crate::chain::Subject::Person(person))
      .await
      .is_ok_and(|answer| crate::chain::flag(answer.as_ref(), false))
  };
  let (network, secrets) =
    futures_util::join!(allows(settings.network.key.as_str()), allows(settings.secrets.key.as_str()));

  Personal { network, secrets }
}

/// What a turn's resolution changed: the apps turned off and those turned on since the file last said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changed {
  pub off: Vec<String>,
  pub on: Vec<String>,
}

/// The person's account a record for `caller` resolves apps for: the account it acts for, where that is a
/// person's account other than the administrator's and the caller holds no administrator's grant; none for the
/// deployment's own callers and the administrator's.
pub async fn person_of(ground: &Ground, caller: Caller<'_>) -> Result<Option<String>, Refusal> {
  let acting = acting_for(&ground.db, &ground.roots.data, caller).await?;
  let Some(account) = acting.account.filter(|_| !acting.administrator) else {
    return Ok(None);
  };

  if crate::account_isolation::owners_own(&ground.db, &account).await? {
    return Ok(None);
  }

  Ok(Some(account))
}

/// The file a resident program's record names, in its place.
pub fn place_file(place: &Path) -> PathBuf {
  place.join(PLACE_FILE)
}

// How many run files this process named, so no two of one moment share a name.
static NAMED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The file a run naming no place has its record name: under the data root, named by when its record lapses, the
/// process naming it, when, and how many that process named before it. The files whose records lapsed before
/// `now` are taken away as the name is made.
pub fn run_file(data_root: &Path, lapses_at: u64, now: u64) -> PathBuf {
  let folder = data_root.join(RUN_FOLDER);

  if let Ok(standing) = std::fs::read_dir(&folder) {
    for file in standing.flatten() {
      let lapsed = file
        .file_name()
        .to_str()
        .and_then(|name| name.split('-').next())
        .and_then(|at| at.parse::<u64>().ok())
        .is_some_and(|at| at < now);

      if lapsed {
        let _ = std::fs::remove_file(file.path());
      }
    }
  }

  folder.join(format!(
    "{lapses_at}-{}-{now}-{}.json",
    std::process::id(),
    NAMED.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
  ))
}

/// The file as it stands, where it reads.
pub fn read(file: &Path) -> Option<Withheld> {
  agent_host::disk::read_json(file)
}

fn write(file: &Path, withheld: &Withheld) -> Result<(), String> {
  if let Some(folder) = file.parent() {
    std::fs::create_dir_all(folder).map_err(|why| format!("{}: {why}", folder.display()))?;
  }

  let text = serde_json::to_string(withheld).map_err(|why| why.to_string())?;

  agent_host::disk::publish_bytes(file, text.as_bytes()).map_err(|why| format!("{}: {why}", file.display()))
}

/// The keys of the apps that do not reach `person`: every app whose offered setting the account service declares,
/// each withheld unless both its settings resolve `true` for the person. An app whose settings do not resolve is
/// withheld; an account service that does not list its kinds is the error.
pub async fn resolve(service: &AccountService, person: &str) -> Result<BTreeSet<String>, String> {
  if service.address.is_empty() {
    return Err("no account service is named (WEB_ADDRESS)".to_owned());
  }

  let client = reqwest::Client::new();
  let listed = get(&client, service, &format!("{}/chain/kinds", service.address), &[]).await?;
  let apps: Vec<String> = listed["kinds"]
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(|kind| kind["key"].as_str())
    .filter_map(|key| key.strip_prefix(KIND_HEAD)?.strip_suffix(OFFERED_TAIL))
    .map(str::to_owned)
    .collect();
  let client = &client;
  let asks = apps.iter().map(|app| async move {
    let reaches = |setting: &'static str| {
      let address = format!("{}/chain/kinds/{KIND_HEAD}{app}.{setting}/resolve", service.address);

      async move {
        get(client, service, &address, &[("person", person)])
          .await
          .is_ok_and(|resolved| resolved["value"] == Value::Bool(true))
      }
    };
    let (offered, on) = futures_util::join!(reaches("offered"), reaches("on"));

    (app.clone(), offered && on)
  });

  Ok(
    join_all(asks)
      .await
      .into_iter()
      .filter(|(_, reaches)| !reaches)
      .map(|(app, _)| app)
      .collect(),
  )
}

// One read of the account service, answered as JSON where it answers 200.
async fn get(
  client: &reqwest::Client,
  service: &AccountService,
  address: &str,
  query: &[(&str, &str)],
) -> Result<Value, String> {
  let answer = client
    .get(address)
    .bearer_auth(&service.token)
    .query(query)
    .send()
    .await
    .map_err(|why| format!("the account service was not reached: {why}"))?;
  let status = answer.status();

  if !status.is_success() {
    return Err(format!("the account service answered {}", status.as_u16()));
  }

  answer
    .json()
    .await
    .map_err(|why| format!("the account service's answer does not read: {why}"))
}

/// The account service as agency reaches it.
pub fn service(ground: &Ground) -> AccountService {
  AccountService::read(&ground.roots.data, |name| ground.vars().get(name).cloned())
}

/// The file written for `person` before a record naming it is issued; where the account service cannot be asked,
/// left as it stands, which a file never written yet reads to the tool host as every app
/// withheld.
pub async fn write_for(ground: &Ground, file: &Path, person: &str, issued: &Issued) {
  let service = service(ground);

  match resolve_all(ground, &service, person, issued).await {
    Ok(apps) => {
      let withheld = Withheld {
        apps,
        account: person.to_owned(),
        personal: personal(&service, person).await,
        holder: issued.holder.clone(),
        line: issued.line.clone(),
      };

      if let Err(why) = write(file, &withheld) {
        eprintln!("the withheld apps were not written: {why}");
      }
    }
    Err(why) => {
      eprintln!("the apps of account {person} were not resolved, so its file stands as it was: {why}")
    }
  }
}

// The apps the chain withholds from the person, and the MCP servers of theirs the record's holder is not attached to.
async fn resolve_all(
  ground: &Ground,
  service: &AccountService,
  person: &str,
  issued: &Issued,
) -> Result<BTreeSet<String>, String> {
  let mut apps = resolve(service, person).await?;

  // A registry that cannot be read names no MCP server, which the tool host then cannot serve either.
  match crate::attachments::withheld_mcp(ground, person, issued.holder.as_ref(), issued.line.as_deref())
    .await
  {
    Ok(mcp) => apps.extend(mcp),
    Err(why) => eprintln!("the MCP servers of account {person} were not read: {why}"),
  }

  Ok(apps)
}

/// The file written for a record of the administrator's account: the keys of its MCP servers the record's holder is
/// not attached to alone, and `personal` both true, so its sessions keep every other app. Whether it was written: a
/// record whose file was not names none.
pub async fn write_for_administrator(ground: &Ground, file: &Path, account: &str, issued: &Issued) -> bool {
  match crate::attachments::withheld_mcp(ground, account, issued.holder.as_ref(), issued.line.as_deref())
    .await
  {
    Ok(apps) => {
      let withheld = Withheld {
        apps,
        account: account.to_owned(),
        personal: Personal {
          network: true,
          secrets: true,
        },
        holder: issued.holder.clone(),
        line: issued.line.clone(),
      };

      match write(file, &withheld) {
        Ok(()) => true,
        Err(why) => {
          eprintln!("the withheld apps were not written: {why}");
          false
        }
      }
    }
    Err(why) => {
      eprintln!("the MCP servers of account {account} were not read, so its record names no file: {why}");
      false
    }
  }
}

/// As a resident program's turn begins: the place's file, where it stands, resolved again for the account it
/// names, and how its apps changed, none where they did not; what the person's personal tools reach is written
/// again with them. Nothing changes where the account service cannot be asked.
pub async fn refresh(ground: &Ground, place: &Path) -> Option<Changed> {
  let file = place_file(place);
  let before = read(&file)?;
  let service = service(ground);
  let issued = Issued {
    holder: before.holder.clone(),
    line: before.line.clone(),
  };
  let administrator =
    crate::account_isolation::owners_own(&ground.db, &before.account).await.unwrap_or(false);
  let resolved = if administrator {
    crate::attachments::withheld_mcp(
      ground,
      &before.account,
      issued.holder.as_ref(),
      issued.line.as_deref(),
    )
    .await
  } else {
    resolve_all(ground, &service, &before.account, &issued).await
  };
  let apps = match resolved {
    Ok(apps) => apps,
    Err(why) => {
      eprintln!(
        "the apps of account {} were not resolved, so its file stands as it was: {why}",
        before.account
      );
      return None;
    }
  };

  let personal = if administrator {
    Personal {
      network: true,
      secrets: true,
    }
  } else {
    personal(&service, &before.account).await
  };

  if apps == before.apps && personal == before.personal {
    return None;
  }

  let changed = Changed {
    off: apps.difference(&before.apps).cloned().collect(),
    on: before.apps.difference(&apps).cloned().collect(),
  };
  let apps_moved = apps != before.apps;

  if let Err(why) = write(
    &file,
    &Withheld {
      apps,
      personal,
      ..before
    },
  ) {
    eprintln!("the withheld apps were not written: {why}");
    return None;
  }

  apps_moved.then_some(changed)
}

/// The fact a session is told of a change of the apps it reaches.
pub fn changed_fact(changed: &Changed) -> String {
  let named = |apps: &[String]| if apps.is_empty() { "none".to_owned() } else { apps.join(", ") };

  policy::fill_marks(
    &policy::chat_line::CHAT_LINE.tools.apps_changed,
    &[("off", &named(&changed.off)), ("on", &named(&changed.on))],
  )
}

/// The instruction a session in a room is told of a change of its apps by: the fact of it, with no instruction.
pub fn changed_instruction(agent: &str, room: i64, changed: &Changed) -> Value {
  serde_json::json!({
    "agentName": agent,
    "room": room,
    "said": { "fact": changed_fact(changed) },
    "sort": "notice",
    "kind": "session-apps",
    "summary": policy::chat_line::CHAT_LINE.tools.apps_changed_summary,
  })
}

/// As a chat program's turn in a room begins: its apps resolved again, and a change owed to its session in agency's
/// outbox, keyed by the agent, the room and the change.
pub async fn turn_begins(ground: &Ground, place: &Path, agent: &str, room: Option<i64>) {
  let Some(changed) = refresh(ground, place).await else {
    return;
  };
  let Some(room) = room else {
    return;
  };
  let notice = crate::outbox::Notice::instruction(
    changed_instruction(agent, room, &changed),
    crate::outbox::Keep::Once,
  );
  let key = format!("{}@{agent}:{room}:{}|{}", notice.key, changed.off.join(","), changed.on.join(","));

  if let Err(why) = crate::outbox::owe(&ground.db, ground.vars(), notice.keyed(key)).await {
    eprintln!("{agent}: the session was not owed the change of its apps: {why}");
  }
}
