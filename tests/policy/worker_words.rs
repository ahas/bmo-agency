use policy::keepalive::KEEPALIVE;
use policy::worker_words::*;

#[test]
fn a_word_s_clock_outlasts_a_ping_and_a_held_word_waits_longer_than_a_round() {
  assert!(WORKER_WORDS.answer_minutes * 60 > KEEPALIVE.worker_ping_seconds);
  assert!(WORKER_WORDS.person_wait_seconds > WORKER_WORDS.tick_seconds);
  assert!(WORKER_WORDS.title.contains("{worker}") && WORKER_WORDS.what.contains("{worker}"));
  assert!(WORKER_WORDS.ended.cut.contains("{clock}") && WORKER_WORDS.ended.cut.contains("{cut}"));
}
