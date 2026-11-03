//! A supervisor's system prompt: the role whole and the persona, the base prompt's values, and the persona read
//! out of the prompt folder its row names rather than one named after it.

use std::path::Path;

use harness_machine::supervisor_prompt::*;
use policy::base_prompts::BASE_PROMPTS;
use policy::supervisors::SUPERVISORS;

// The supervisor every test composes for: its name, and the prompt folder its row names, which differs
// from the name so a prompt read by the name would show.
const PAL: Supervisor = Supervisor {
  name: "tester-pal",
  prompts: Some("pal-prompts"),
};

// A tree of agent documents, each naming what it is, and the roots over it.
fn roots(dir: &Path) -> Roots {
  let write = |path: &str, text: &str| {
    let at = dir.join("assets").join(path);

    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, format!("{text}\n")).unwrap();
  };

  write("prompts/harness.md", "the platform's harness");
  write("prompts/rules/self-preservation.md", "keep the system");
  write("prompts/rules/absolute-rules.md", "ABSOLUTE RULES");
  write("prompts/rules/agent-words.md", "agent words");
  write("prompts/agents/pieces/frame.md", "frame\n{{tools}}\n{{unknown}}");
  write("prompts/agents/pieces/turn.md", "turn");
  write("prompts/agents/pieces/tools.md", "tools at {{workplace}}, verbs in {{tree}}");
  write("prompts/agents/pal-prompts/persona.md", "persona file");
  write("prompts/agents/tester-pal/persona.md", "the name's persona, never read");

  Roots {
    assets: dir.join("assets"),
    data: dir.join("data"),
  }
}

#[test]
fn a_supervisor_whose_row_names_no_prompt_folder_is_sent_no_persona() {
  let dir = tempfile::tempdir().unwrap();
  let bare = Supervisor {
    name: "tester-pal",
    prompts: None,
  };
  let composed = system(&roots(dir.path()), bare, &Composition::default(), None, true).unwrap();

  assert!(composed.ends_with("RULEBOOK prompts/rules/agent-words.md:\nagent words"), "{composed}");
}

#[test]
fn a_prompt_folder_holding_no_persona_sends_none() {
  let dir = tempfile::tempdir().unwrap();
  let roots = roots(dir.path());

  assert_eq!(roots.persona(Some("empty-folder"), None).unwrap(), "");
  assert_eq!(roots.persona(Some("pal-prompts"), Some("held")).unwrap(), "held");
}

#[test]
fn a_prompt_folder_that_is_not_one_plain_name_is_refused() {
  let dir = tempfile::tempdir().unwrap();
  let roots = roots(dir.path());

  for folder in ["", "..", "../rules", "pal-prompts/inner", "/etc"] {
    assert!(roots.prompt_folder(folder).is_err(), "{folder:?}");
    assert!(roots.persona(Some(folder), None).is_err(), "{folder:?}");
  }

  assert_eq!(
    roots.prompt_folder("pal-prompts").unwrap(),
    dir.path().join("assets/prompts/agents/pal-prompts")
  );
}

#[test]
fn every_policy_piece_has_its_own_text() {
  let dir = tempfile::tempdir().unwrap();
  let roots = roots(dir.path());

  for name in SUPERVISORS.pieces_role.iter().map(String::as_str).chain(["persona"]) {
    assert!(piece(&roots, PAL, name, &Composition::default()).is_ok(), "{name}");
  }
}

// On a program that is a harness of its own and with no base chosen, the prompt is the role whole and the
// persona, with no opening of the platform's.
#[test]
fn the_prompt_is_the_rulebook_the_role_whole_then_the_persona() {
  let dir = tempfile::tempdir().unwrap();
  let composed = system(&roots(dir.path()), PAL, &Composition::default(), None, false).unwrap();
  let home = dir.path().join("data/supervisor-homes/tester-pal").display().to_string();
  let tree = dir.path().join("assets").display().to_string();

  assert_eq!(
    composed,
    format!(
      "RULEBOOK prompts/rules/self-preservation.md:\nkeep the system\n\nABSOLUTE RULES\n\nframe\ntools at {home}, verbs in {tree}\n{{{{unknown}}}}\n\nturn\n\nRULEBOOK prompts/rules/agent-words.md:\nagent words\n\npersona file"
    )
  );
}

// On Claude Code a supervisor that chooses no base opens on the platform's harness opening, read out of the
// asset root, before the role and the persona (docs/supervisors.md, 「What a turn is given」).
#[test]
fn a_supervisor_with_no_base_opens_on_the_platform_s_harness() {
  let dir = tempfile::tempdir().unwrap();
  let roots = roots(dir.path());
  let opened = system(&roots, PAL, &Composition::default(), None, true).unwrap();
  let bare = system(&roots, PAL, &Composition::default(), None, false).unwrap();

  assert_eq!(roots.harness().unwrap(), "the platform's harness");
  assert_eq!(opened, format!("the platform's harness\n\n{bare}"));
}

#[test]
fn the_base_prompt_is_the_harness_and_the_settings_persona_closes_the_prompt() {
  let dir = tempfile::tempdir().unwrap();
  let roots = roots(dir.path());
  let settings = Composition {
    persona: Some("short".into()),
  };
  let base = Base {
    text: "\nRun in {{cwd}} on {{model}} for {{nobody}}.\n".into(),
    model: "model-0-1".into(),
  };
  let home = dir.path().join("data/supervisor-homes/tester-pal").display().to_string();
  let tree = dir.path().join("assets").display().to_string();

  // The chosen base stands in the platform's opening's place, so the opening is not sent beside it.
  assert_eq!(
    system(&roots, PAL, &settings, Some(&base), true).unwrap(),
    format!(
      "Run in {home} on model-0-1 for {{{{nobody}}}}.\n\nRULEBOOK prompts/rules/self-preservation.md:\nkeep the system\n\nABSOLUTE RULES\n\nframe\ntools at {home}, verbs in {tree}\n{{{{unknown}}}}\n\nturn\n\nRULEBOOK prompts/rules/agent-words.md:\nagent words\n\nshort"
    )
  );
}

#[test]
fn every_base_prompt_value_has_a_reading_on_this_side() {
  for name in &BASE_PROMPTS.values {
    assert!(base_value(name, Path::new("/w"), "m").is_some(), "{name}");
  }

  assert!(machine::local_date().len() == 10 && machine::os_version().contains(' '));
}
