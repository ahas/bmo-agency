//! What a turn's text carries: the data of what changed, and no sentence telling the model what to do but the one
//! next-step line a queue notice says under its data (docs/agent-control.md, 「Every queue notice says its next
//! step」), with the queue's standing under that.

// The sentences a turn used to carry, before they moved into the session's system prompt. A notice that
// brings one back has been written wrong even where its data still stands.
const INSTRUCTIONS: &[&str] = &[
  "Say in one line",
  "Correct ",
  "Read it with",
  "Read the item",
  "Write its",
  "put it to the owner",
  "never answer it",
  "launch it",
  "lease it",
  "run it",
  "Judge it",
  "approve it",
  "give lease",
  "Relay its",
  "Relay it",
  "never paste",
  "Do not ",
  "Nothing is asked",
  "file the malfunction",
  "reach any of them",
  "Nothing needs saying",
  "tell the owner",
  "why not",
  "job_lease_next",
  "assign_worker",
  "run_job",
  "job_item_update",
  "job_question",
  "job_replies",
];

/// Fails where `text` carries a sentence telling the model what to do rather than what changed, a queue notice's
/// next-step line and the standing under it left aside: they are its one instruction, said by the policy apart from
/// its data.
pub fn assert_no_instruction(text: &str) {
  let data = text.split("\nNext: ").next().unwrap_or(text);

  for phrase in INSTRUCTIONS {
    assert!(!data.contains(phrase), "{phrase:?} in {data}");
  }
}
