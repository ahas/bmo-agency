use crate::support::wording::assert_no_instruction;
use policy::job_notices::*;

#[test]
fn the_orderer_chain_is_bounded_and_every_order_carries_its_data_and_no_instruction() {
  assert!(JOB_NOTICES.orderer_depth > 0);
  assert!(!JOB_NOTICES.any_worker.is_empty());
  assert_eq!(JOB_NOTICES.wait_labels, ["follows", "waiting-on"]);

  for (order, marks) in [
    (&JOB_NOTICES.paused_order, &["{item}", "{title}", "{queue}"][..]),
    (&JOB_NOTICES.unlaunchable_order, &["{item}", "{title}", "{assignee}"][..]),
    (&JOB_NOTICES.agent_order, &["{item}", "{title}", "{label}", "{why}"][..]),
    (
      &JOB_NOTICES.moved_order,
      &["{item}", "{title}", "{state}", "{merged}", "{job}", "{worker}"][..],
    ),
    (&JOB_NOTICES.closed_order, &["{item}", "{title}", "{by}"][..]),
    (&JOB_NOTICES.wait_ended_order, &["{item}", "{title}", "{label}", "{waited}"][..]),
    (&JOB_NOTICES.waited_item, &["{item}", "{title}", "{state}"][..]),
    (&JOB_NOTICES.question_order, &["{question}", "{item}", "{text}", "{choices}"][..]),
    (&JOB_NOTICES.waiting_order, &["{item}", "{worker}", "{why}"][..]),
    (&JOB_NOTICES.close_order, &["{item}", "{lease}"][..]),
  ] {
    for mark in marks {
      assert!(order.contains(mark), "{mark} in {order}");
    }

    assert_no_instruction(order);
  }

  assert!(!JOB_NOTICES.question_due_code.is_empty() && !JOB_NOTICES.failed_code.is_empty());
}
