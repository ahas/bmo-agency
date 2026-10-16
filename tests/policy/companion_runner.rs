use policy::companion_runner::*;

#[test]
fn reads_and_marks_what_each_line_carries() {
  let runner = &*COMPANION_RUNNER;

  assert_eq!(runner.limits.ms(), 30 * 60_000);
  assert!(runner.cut.notice.contains("{reason}"));
  assert!(runner.blocks.reference_opening.contains("{count}"));
  assert_eq!(runner.cut.instruction.len(), 4);
}
