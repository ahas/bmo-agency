use agency_server::stand_in::*;

fn standing(order: &[&str], closed: bool) -> Standing {
  Standing {
    route: None,
    order: order.iter().map(|one| (*one).to_owned()).collect(),
    codex_present: true,
    openrouter_present: false,
    services: vec![Service {
      provider: "registered".to_owned(),
      present: true,
      walked: false,
    }],
    limit_rest: true,
    accounts: vec![(true, Some(if closed { 0.99 } else { 0.2 }))],
  }
}

#[test]
fn a_stand_in_above_claude_answers_and_one_below_only_once_every_login_is_closed() {
  assert!(meets(&standing(&["codex", CLAUDE], false)));
  assert!(!meets(&standing(&[CLAUDE, "codex"], false)));
  assert!(meets(&standing(&[CLAUDE, "codex"], true)));
  // A registration the key does not walk is passed by, and OpenRouter with no key offers nothing.
  assert!(!meets(&standing(&["registered", "openrouter", CLAUDE], false)));
}

#[test]
fn the_walk_asks_the_stand_ins_above_claude_and_must_answer_where_claude_is_out_of_it() {
  let walked = walk_over(&standing(&["codex", CLAUDE], false)).expect("a walk");

  assert_eq!(
    (walked.steps, walked.required, walked.chosen.as_str()),
    (vec!["codex".to_owned()], false, "auto")
  );
  assert_eq!(walk_over(&standing(&[CLAUDE, "codex"], false)), None);

  let mut pinned = standing(&[CLAUDE, "codex"], false);

  pinned.route = Some(CLAUDE.to_owned());
  assert_eq!(walk_over(&pinned), None);
  pinned.order = vec!["codex".to_owned()];
  assert_eq!(walk_over(&pinned).map(|walked| walked.required), Some(true));
  pinned.route = Some("codex".to_owned());
  assert_eq!(walk_over(&pinned).map(|walked| walked.steps), Some(vec!["codex".to_owned()]));
}

#[test]
fn a_key_set_to_one_provider_answers_there_alone() {
  let mut set = standing(&[CLAUDE, "codex"], false);

  set.route = Some("codex".to_owned());
  assert!(meets(&set));
  set.route = Some(CLAUDE.to_owned());
  assert!(!meets(&set));
}

// The worker/job line crosses over on this same walk (docs/cache-fallback.md): while every Claude login
// rests past its weekly ceiling the walk falls to DeepSeek on its own, and once Claude can answer again the
// key stands back on Claude's own road with no stand-in.
#[test]
fn a_worker_key_falls_to_a_stand_in_while_claude_rests_and_back_to_claude_when_it_can_answer() {
  let with_deepseek = |closed: bool| {
    let mut standing = standing(&[CLAUDE, "deepseek"], closed);

    standing.services.push(Service {
      provider: "deepseek".to_owned(),
      present: true,
      walked: true,
    });
    standing
  };
  let walked = walk_over(&with_deepseek(true)).expect("a walk while Claude rests");

  assert_eq!(
    (walked.steps, walked.required, walked.chosen.as_str()),
    (vec!["deepseek".to_owned()], false, "auto")
  );
  assert_eq!(walk_over(&with_deepseek(false)), None);
}
