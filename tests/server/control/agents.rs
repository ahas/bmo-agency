use agency_server::control::agents::*;

#[test]
fn reads_a_job_choice_as_the_launcher_names_one() {
  let choice = job_choice("claude-code::claude-opus-5-5").unwrap();

  assert_eq!(
    (choice.provider.as_str(), choice.model.as_deref()),
    ("claude-code", Some("claude-opus-5-5"))
  );
  assert!(job_choice("nobody::model").is_none());
  assert!(job_choice("codex::").is_none());
  assert!(job_choice("claude-opus-5-5").is_none());
}
