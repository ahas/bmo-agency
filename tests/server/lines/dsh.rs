use agency_server::lines::dsh::*;
use agency_server::lines::ordered::Ordered;
use agency_server::lines::token::TOKEN_VARIABLE;

#[test]
fn a_long_task_is_cut_at_spaces_that_join_it_again() {
  // Three bytes and one UTF-16 unit a letter, so the first stretch alone runs past one argument's bytes.
  let task = format!("{} flag -x {}", "€".repeat(WORD_UNITS - 3), "b".repeat(10));
  let words = task_words(&task);

  assert_eq!(words.len(), 2);
  assert!(words.iter().all(|word| word.len() <= WORD_BYTES && !word.starts_with('-')));
  assert_eq!(words.join(" "), task);
  assert_eq!(task_words("€ task\0"), vec!["€ task".to_owned()]);
}

#[test]
fn a_failure_is_dsh_s_own_word_before_the_last_line() {
  assert_eq!(
    failure("dsh: reasoning about it\nError: dsh: no route\nbanner", 1),
    "Error: dsh: no route"
  );
  assert_eq!(failure("dsh: reasoning\nnpm ERR! fetch failed\n", 2), "exit 2: npm ERR! fetch failed");
  assert_eq!(failure("", 3), "exit 3");
}

#[test]
fn the_overlay_writes_the_bearer_as_dsh_reads_it() {
  let overlay =
    Ordered::map(vec![("Authorization", Ordered::Text(format!("{ENVIRONMENT_MARK}{TOKEN_VARIABLE}")))]);

  assert_eq!(
    overlay_text(&overlay).unwrap(),
    "{\n  \"Authorization\": !!js \"`Bearer ${process.env.BMO_MCP_TOKEN}`\"\n}\n"
  );
}
