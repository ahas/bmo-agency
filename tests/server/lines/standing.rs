use harness_providers::Provider;
use harness_providers::claude_code::ClaudeCode;

use agency_server::lines::standing::{fold_share, gateway_headers};

#[test]
fn a_run_on_the_gateway_names_its_wait_its_sampling_and_its_programs_long_tools() {
  let tools = ClaudeCode.long_tools();

  assert_eq!(tools, ["Bash", "Agent", "Task", "WebFetch", "WebSearch"]);
  assert_eq!(
    gateway_headers(90_500, Some(r#"{"top_k":20}"#), tools),
    [
      "x-litellm-timeout: 91",
      "x-litellm-stream-timeout: 91",
      r#"x-bmo-sampling: {"top_k":20}"#,
      "x-bmo-long-tools: Bash,Agent,Task,WebFetch,WebSearch",
    ]
  );
  assert_eq!(gateway_headers(1, None, &[]), ["x-litellm-timeout: 1", "x-litellm-stream-timeout: 1"]);
}

#[test]
fn the_fold_share_is_what_the_answer_ceiling_leaves_of_the_window_rounded_down() {
  assert_eq!(fold_share(262_144, 32_768), "87.5000");
  assert_eq!(fold_share(3, 1), "66.6666");
  assert_eq!(fold_share(65_536, 0), "100.0000");
  assert_eq!(fold_share(4_096, 8_192), "0.0001");
}
