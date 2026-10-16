use agency_server::door::looks::*;

#[test]
fn the_trust_question_is_accepted_by_moving_down_from_the_cursor_to_the_accepting_row() {
  let screen = "Do you trust the files in this folder?\n\n ❯ 1. Yes, I trust this folder\n   2. No, exit\n";

  assert_eq!(folder_trust_keys(screen), Some(vec!["enter"]));

  let screen = " ❯ 1. No, exit\n   2. Yes, I trust this folder\n";

  assert_eq!(folder_trust_keys(screen), Some(vec!["down", "enter"]));
  assert_eq!(folder_trust_keys("❯ a prompt box\n"), None);
  assert_eq!(folder_trust_keys("   1. No, exit\n   2. Yes, I trust this folder\n"), None);
}
