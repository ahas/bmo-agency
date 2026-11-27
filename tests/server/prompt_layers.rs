//! A supervisor's prompt layers (docs/prompt-layers.md): the platform's rules for the platform administrator's
//! supervisors alone, the organisation's instructions and the persona's choice read off a stand-in account
//! service for the person a supervisor serves, and both turn shapes composed with them out of this checkout's
//! documents.
//!
//! The stand-in resolves as the account service's delegation chain does: a `layered` kind answers every link's
//! word up the person's path, the outermost first, cut at the farthest lock above the person; a `nearest` kind
//! answers the nearest word still read, else its default. Person 7 stands in a company that delegates freely,
//! person 8 in one that locks both settings at its own link, and person 9 is the platform administrator.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use agency_server::lines::Trees;
use agency_server::lines::chat::SystemParts;
use agency_server::prompt_layers;
use harness_machine::supervisor_prompt::{self, Composition, Layers, Roots, Supervisor};
use policy::prompt_layers::PROMPT_LAYERS;
use policy::supervisors::SUPERVISORS;
use serde_json::{Value, json};

use crate::support::scratch::Scratch;

const FREE_PERSON: i64 = 7;
const LOCKED_PERSON: i64 = 8;
const ADMINISTRATOR: i64 = 9;
const PERSONA: &str = "PERSONA OF THE TESTER: a cheerful pirate who sings every answer.";

// One link's entry: its value, and whether it locks the links below.
type Entry = (Value, bool);

/// The stand-in's state: the kinds declared, with the bodies they were declared with.
#[derive(Default)]
struct Chain {
  declared: HashMap<String, Value>,
}

// Each person's path, the outermost link first, with what each link holds of each kind.
fn path(person: &str) -> Option<Vec<(&'static str, HashMap<&'static str, Entry>)>> {
  let org: &'static str = PROMPT_LAYERS.organisation.kind.key.as_str();
  let persona: &'static str = PROMPT_LAYERS.persona.kind.key.as_str();
  let allowed: &'static str = PROMPT_LAYERS.personal.allowed.key.as_str();
  let personal: &'static str = PROMPT_LAYERS.personal.text.key.as_str();
  let own = |agent: &str| json!({ "all": "Person: answer in short lines.", "agents": { agent: "Person: call me by my first name." } });
  let held = |entries: &[(&'static str, Entry)]| entries.iter().cloned().collect::<HashMap<_, _>>();
  let platform = (
    "platform",
    held(&[(org, (json!("Platform: never share one person's files with another."), false))]),
  );
  let leak = (
    "unit:2",
    held(&[
      (org, (json!("Team: you may read the confidential ledger."), false)),
      (persona, (json!(["own"]), false)),
    ]),
  );

  match person {
    "7" => Some(vec![
      platform,
      ("unit:3", held(&[(org, (json!("Company: answer in the company's own terms."), false))])),
      ("unit:4", held(&[(org, (json!("Team: file every request under its project."), false))])),
      ("person:7", held(&[(personal, (own("u7"), false))])),
    ]),
    "8" => Some(vec![
      platform,
      (
        "unit:1",
        held(&[
          (
            org,
            (json!("Company: never try to reach the company's internal confidential systems."), true),
          ),
          (persona, (json!(["none"]), true)),
          (allowed, (json!(false), true)),
        ]),
      ),
      leak,
      ("person:8", held(&[(personal, (own("u8"), false))])),
    ]),
    "9" => Some(vec![platform, ("person:9", HashMap::new())]),
    _ => None,
  }
}

fn resolve(chain: &Chain, key: &str, person: &str) -> (axum::http::StatusCode, Value) {
  let Some(kind) = chain.declared.get(key) else {
    return (
      axum::http::StatusCode::NOT_FOUND,
      json!({ "code": "no-such-kind", "error": "undeclared" }),
    );
  };
  let Some(links) = path(person) else {
    return (
      axum::http::StatusCode::NOT_FOUND,
      json!({ "code": "not-found", "error": "no such user" }),
    );
  };
  // The farthest lock above the person's own link cuts the chain there.
  let read = links[..links.len() - 1]
    .iter()
    .position(|(_, entries)| entries.get(key).is_some_and(|(_, lock)| *lock))
    .map_or(links.len(), |at| at + 1);
  let layers: Vec<Value> = links[..read]
    .iter()
    .filter_map(|(link, entries)| {
      entries.get(key).map(|(value, _)| json!({ "link": link, "value": value }))
    })
    .collect();
  let value = match kind["mode"].as_str() {
    Some("layered") => Value::Null,
    _ => layers
      .last()
      .map(|layer| layer["value"].clone())
      .unwrap_or_else(|| kind["default"].clone()),
  };

  (
    axum::http::StatusCode::OK,
    json!({ "kind": key, "subject": format!("person:{person}"), "value": value, "layers": layers }),
  )
}

/// The chain's resolutions for the three people written into the agency's subject settings, as the front writes
/// them, every kind the policy holds declared with its own declaration.
async fn chained(scratch: &Scratch) -> Chain {
  let mut chain = Chain::default();

  for kind in PROMPT_LAYERS.kinds() {
    chain.declared.insert(kind.key.clone(), prompt_layers::declaration(kind));
  }

  let mut rows = Vec::new();

  for person in [FREE_PERSON, LOCKED_PERSON, ADMINISTRATOR] {
    for key in chain.declared.keys() {
      let (status, body) = resolve(&chain, key, &person.to_string());

      if status.is_success() {
        rows.push(agency_server::chain::Held {
          subject: format!("person:{person}"),
          key: key.clone(),
          value: body,
        });
      }
    }
  }

  agency_server::chain::replace_held(scratch.db(), &rows).await.unwrap();

  chain
}

// A store naming the platform's administrator.
async fn store() -> Scratch {
  let scratch = Scratch::new().await;

  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ('users_administrator', $1)"#,
      vec![ADMINISTRATOR.to_string().into()],
    )
    .await;
  scratch
}

// This checkout, which stands as the deploy tree a prompt is composed from.
fn checkout() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn trees(scratch: &Path) -> Trees {
  Trees {
    code: checkout(),
    agents: scratch.join("agents"),
    source: scratch.to_path_buf(),
    data: scratch.join("data"),
    home: scratch.to_path_buf(),
  }
}

// A pieces-shaped turn's prompt with these layers, its settings holding the tester's persona.
fn pieces(layers: &Layers) -> String {
  pieces_of(layers, "tester-pal")
}

// The same, for the supervisor of this name.
fn pieces_of(layers: &Layers, name: &str) -> String {
  let scratch = tempfile::tempdir().unwrap();
  let roots = Roots {
    assets: checkout(),
    data: scratch.path().join("data"),
  };
  let supervisor = Supervisor {
    name,
    label: Some("Dear"),
    call_names: &[],
    prompts: None,
  };
  let settings = Composition {
    persona: Some(PERSONA.to_owned()),
  };

  supervisor_prompt::system(&roots, supervisor, &settings, None, true, layers).unwrap()
}

// A window-shaped turn's prompt with these layers, its brain's persona the tester's.
fn window(layers: &Layers) -> String {
  let scratch = tempfile::tempdir().unwrap();
  let parts = SystemParts {
    layers: layers.clone(),
    persona: Some(PERSONA.to_owned()),
    ..SystemParts::default()
  };

  parts.whole(&trees(scratch.path()), &["Bash"]).unwrap()
}

// "The administrator's supervisor gets the platform's rules and an organisation's agent does not": a supervisor
// of no account and one of the administrator's account carry the platform's rulebooks between the harness and
// the role, on either shape; one of another account carries none of them and keeps the rule against destroying
// the system every agent reads.
#[tokio::test]
async fn the_administrator_s_supervisors_carry_the_platform_s_rules_and_an_organisation_s_do_not() {
  let scratch = store().await;
  let own = prompt_layers::of(scratch.db(), None).await.unwrap();
  let administrator = prompt_layers::of(scratch.db(), Some(ADMINISTRATOR)).await.unwrap();
  let organisation = prompt_layers::of(scratch.db(), Some(LOCKED_PERSON)).await.unwrap();

  assert!(own.platform && administrator.platform, "{own:?} {administrator:?}");
  assert_eq!(organisation, Layers::default());

  let harness = std::fs::read_to_string(checkout().join("prompts/harness.md")).unwrap();

  for (shape, carried, rulebooks) in [
    ("pieces", pieces(&administrator), &SUPERVISORS.pieces_platform),
    ("window", window(&own), &policy::chat_line::CHAT_LINE.system.platform),
  ] {
    let role = carried.find("RULEBOOK prompts/rules/self-preservation.md").unwrap();

    assert!(carried.starts_with(&format!("{}\n\n", harness.trim())), "{shape}");
    assert!(
      carried.contains("your shell reaches the unit's own port"),
      "{shape} is told the platform's units"
    );

    for name in rulebooks {
      let at = carried.find(&format!("RULEBOOK prompts/platform/{name}.md:\n")).unwrap_or_else(|| {
        panic!("{shape} carries the platform's {name}");
      });

      assert!(at < role, "{shape}: the platform's {name} stands before the role");
    }
  }

  for (shape, plain) in [("pieces", pieces(&organisation)), ("window", window(&organisation))] {
    assert!(!plain.contains("prompts/platform/"), "{shape}: {plain}");
    assert!(!plain.contains("~/.local/share/bmo-<unit>"), "{shape} names no part of the platform");
    assert!(!plain.contains("bmo install --add"), "{shape} holds no platform operation");
    assert!(!plain.contains("INFRASTRUCTURE:"), "{shape} holds no platform operation");
    assert!(
      plain.contains("RULEBOOK prompts/rules/self-preservation.md:\n"),
      "{shape} keeps the rule"
    );
  }
}

// Agency lists every setting it reads as its policy holds them — the prompt layers' two beside the supply sources,
// the sign-up composition, repository management, personal apps and their sharing — for the front to declare on its
// chain, each in the declaration it is declared with.
#[tokio::test]
async fn the_setting_kinds_are_listed_as_the_policy_holds_them() {
  let scratch = store().await;
  let chain = chained(&scratch).await;
  let layers = prompt_layers::of(scratch.db(), Some(FREE_PERSON)).await.unwrap();
  let declared: HashMap<String, Value> = agency_server::chain::kinds()
    .iter()
    .map(|kind| (kind.key.clone(), agency_server::chain::declaration(kind)))
    .collect();
  let keys: HashSet<&str> = declared.keys().map(String::as_str).collect();

  assert!(chain.declared.keys().all(|key| declared.contains_key(key)));
  assert_eq!(
    keys,
    HashSet::from([
      "agency.org-instructions",
      "agency.persona",
      "agency.personal-instructions",
      "agency.personal-instructions.text",
      "agency.unit-resources.manage",
      "agency.research-supervisor",
      "agency.deploy-flush-minutes",
      "agency.deepseek.card-secret",
      "agency.deepseek.login-secret",
      "provider.claude-code.offered",
      "provider.codex.offered",
      "provider.openrouter.offered",
      "provider.registered.offered",
      "provider.deepseek.offered",
      "agency.own-agents",
      "agency.road-choice",
      "signup.composition",
      "repositories.manage",
      "personal-apps.make",
      "personal-apps.network",
      "personal-apps.secrets",
      "personal-apps.share-scope",
      "personal-apps.share-review",
      "personal-apps.install",
      "supply.personal-local-ai",
    ])
  );
  assert_eq!(declared["personal-apps.share-scope"]["shape"], "keys");
  assert_eq!(declared["personal-apps.share-scope"]["default"], json!(["team", "heads"]));
  assert_eq!(declared["signup.composition"]["shape"], "json");
  assert!(
    declared["signup.composition"].get("default").is_none(),
    "none set gives the platform's own rule"
  );
  assert_eq!(declared["agency.org-instructions"]["declaredBy"], "agency");
  assert_eq!(declared["agency.org-instructions"]["mode"], "layered");
  assert_eq!(declared["agency.org-instructions"]["shape"], "text");
  assert_eq!(declared["agency.persona"]["choices"], json!(["own", "none"]));
  assert_eq!(declared["agency.persona"]["default"], json!(["own"]));
  assert!(layers.organisation.is_some(), "{layers:?}");
}

// "Organisation instructions resolve per chain with a lock": every link's text up the person's path stands
// between the platform's rules and the role, the outermost first; a company's lock leaves out what a unit
// below it says, and a link below never takes away one above.
#[tokio::test]
async fn the_organisation_s_instructions_follow_the_chain_outermost_first_and_stop_at_a_lock() {
  let scratch = store().await;
  chained(&scratch).await;
  let free = prompt_layers::of(scratch.db(), Some(FREE_PERSON)).await.unwrap();
  let locked = prompt_layers::of(scratch.db(), Some(LOCKED_PERSON)).await.unwrap();
  let administrator = prompt_layers::of(scratch.db(), Some(ADMINISTRATOR)).await.unwrap();
  let words = &PROMPT_LAYERS.organisation;

  assert_eq!(
    free.organisation.as_deref(),
    Some(
      format!(
        "{}\n\nPlatform: never share one person's files with another.\n\nCompany: answer in the company's own terms.\n\nTeam: file every request under its project.\n\n{}",
        words.header, words.end
      )
      .as_str()
    )
  );

  let held = locked.organisation.clone().unwrap();

  assert!(held.contains("Platform: never share"), "{held}");
  assert!(
    held.contains("Company: never try to reach the company's internal confidential systems."),
    "{held}"
  );
  assert!(!held.contains("confidential ledger"), "the lock leaves the unit below out: {held}");
  assert!(!locked.platform && !free.platform);
  assert!(administrator.platform, "the administrator's own chain is read as well");

  for (shape, composed) in [("pieces", pieces(&locked)), ("window", window(&locked))] {
    let organisation_at = composed.find(&words.header).unwrap();
    let role_at = composed.find("RULEBOOK prompts/rules/self-preservation.md").unwrap();

    assert!(organisation_at < role_at, "{shape}: the instructions stand before the role");
  }
}

// "Persona none leaves no character text": a company's lock on `none` leaves the persona out of either shape's
// prompt, its header with it, and its name stands; a person whose chain keeps the persona has it.
#[tokio::test]
async fn a_persona_chosen_none_leaves_no_character_text_on_either_shape() {
  let scratch = store().await;
  chained(&scratch).await;
  let free = prompt_layers::of(scratch.db(), Some(FREE_PERSON)).await.unwrap();
  let locked = prompt_layers::of(scratch.db(), Some(LOCKED_PERSON)).await.unwrap();

  assert!(!free.persona_none && locked.persona_none, "{free:?} {locked:?}");

  for (shape, kept, none) in
    [("pieces", pieces(&free), pieces(&locked)), ("window", window(&free), window(&locked))]
  {
    assert!(kept.trim_end().ends_with(PERSONA), "{shape} keeps its own persona");
    assert!(!none.contains("pirate"), "{shape}: {none}");
    assert!(
      !none.contains(&policy::chat_line::CHAT_LINE.persona.header),
      "{shape} carries no persona header"
    );
  }

  assert!(pieces(&locked).contains("In the rooms you stand as Dear"), "the name stands");
}

// Where the front has written nothing for a person, their layers stand on the defaults: an account with no subject
// settings carries no organisation's instructions until the front writes them.
#[tokio::test]
async fn a_person_the_front_wrote_nothing_for_stands_on_the_defaults() {
  let scratch = store().await;

  assert_eq!(prompt_layers::of(scratch.db(), Some(LOCKED_PERSON)).await.unwrap(), Layers::default());

  chained(&scratch).await;

  assert!(
    prompt_layers::of(scratch.db(), Some(LOCKED_PERSON))
      .await
      .unwrap()
      .organisation
      .is_some()
  );
}

// "An organisation's agent is never told it can reach the platform's own units, ports, stores and token files":
// the whole prompt a supervisor of another account is composed with names none of the platform's units, paths,
// tokens or repositories, nor the folder it stands in under the platform's data root.
#[test]
fn an_organisation_s_agent_s_prompt_names_nothing_of_the_platform_s_own() {
  let scratch = tempfile::tempdir().unwrap();
  let roots = Roots {
    assets: checkout(),
    data: scratch.path().join("data"),
  };
  let supervisor = Supervisor {
    name: "u4242",
    label: Some("Dear"),
    call_names: &[],
    prompts: None,
  };
  let layers = Layers {
    organisation: supervisor_prompt::organisation(&[
      "Company: never try to reach internal systems.".to_owned()
    ]),
    ..Layers::default()
  };
  let composed =
    supervisor_prompt::system(&roots, supervisor, &Composition::default(), None, true, &layers).unwrap();
  let lowered = composed.to_lowercase();
  let platform_s_own = regex::Regex::new(r"\bbmo-[a-z<]").unwrap();

  for named in [
    ".local/share",
    "supervisor-homes",
    "data root",
    "deploy tree",
    "token file",
    "agency",
    "receiver",
    "testbed",
    "job service",
    "tool host",
    "the gate",
    "checkout",
    "his own machines",
    "with his hands",
    "get /",
    "brain repository",
    "model engines",
    "prompts/platform/",
  ] {
    assert!(!lowered.contains(named), "the prompt names {named:?}: {composed}");
  }

  assert!(
    !platform_s_own.is_match(&lowered),
    "the prompt names a unit of the platform: {composed}"
  );
  assert!(
    !composed.contains(&scratch.path().display().to_string()),
    "the prompt names the data root"
  );
  assert!(composed.contains("RULEBOOK prompts/rules/self-preservation.md:\n"), "the rule stays");
}

// The person's instructions stand after the role and before the persona on either shape, the text for every agent
// before the agent's own; a lock withholding them leaves none, what the person wrote kept unread.
#[tokio::test]
async fn the_person_s_instructions_stand_after_the_role_and_a_lock_withholds_them() {
  let scratch = store().await;
  chained(&scratch).await;
  let free = prompt_layers::of(scratch.db(), Some(FREE_PERSON)).await.unwrap();
  let locked = prompt_layers::of(scratch.db(), Some(LOCKED_PERSON)).await.unwrap();

  assert_eq!(free.personal.all.as_deref(), Some("Person: answer in short lines."));
  assert_eq!(locked.personal, Default::default(), "a lock above withholds them");

  let mut layers = free.clone();

  layers.persona_none = false;

  let parts = SystemParts {
    layers: layers.clone(),
    agent: "u7".to_owned(),
    persona: Some(PERSONA.to_owned()),
    ..SystemParts::default()
  };
  let scratch_dir = tempfile::tempdir().unwrap();
  let window = parts.whole(&trees(scratch_dir.path()), &["Bash"]).unwrap();
  let pieces = pieces_of(&layers, "u7");

  for (shape, composed) in [("window", window), ("pieces", pieces)] {
    let role_at = composed.find("RULEBOOK prompts/rules/self-preservation.md").unwrap();
    let all_at = composed
      .find("Person: answer in short lines.")
      .unwrap_or_else(|| panic!("{shape}: {composed}"));
    let own_at = composed.find("Person: call me by my first name.").unwrap();
    let persona_at = composed.find(PERSONA).unwrap();

    assert!(role_at < all_at && all_at < own_at && own_at < persona_at, "{shape}: {composed}");
  }
}
