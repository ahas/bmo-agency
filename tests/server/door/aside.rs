//! Words offered to a run's working turn: taken only while its door listens, answered with what became of
//! them, withdrawn where no look took them, refused as the door stops listening, and carried by a later run
//! of the same turn until its session lets the run go.

use agency_server::door::aside;
use agency_server::door::turn::AsideOutcome;

#[tokio::test]
async fn an_offer_is_taken_while_the_door_listens_and_answered_with_its_outcome() {
  assert!(aside::offer("aside-unheard", "words").is_none(), "no door listens for the run");

  let listening = aside::listen("aside-heard");
  let offer = aside::offer("aside-heard", "more words").unwrap();
  let taken = listening.take().unwrap();

  assert_eq!(taken.words, "more words");
  assert!(!offer.withdraw(), "the door has it");

  listening.said(taken, AsideOutcome::Delivered);

  assert_eq!(offer.answer.await.unwrap(), AsideOutcome::Delivered);
  assert_eq!(aside::carried("aside-heard", "the turn"), "the turn\n\nmore words");

  drop(listening);
  assert_eq!(aside::carried("aside-heard", "the turn"), "the turn\n\nmore words", "kept for a rerun");
  aside::forget("aside-heard");
  assert_eq!(aside::carried("aside-heard", "the turn"), "the turn");
}

#[tokio::test]
async fn an_offer_no_look_took_is_withdrawn_or_refused_as_the_door_stops() {
  let listening = aside::listen("aside-untaken");
  let withdrawn = aside::offer("aside-untaken", "first").unwrap();

  assert!(withdrawn.withdraw());
  assert!(listening.take().is_none());

  let waiting = aside::offer("aside-untaken", "second").unwrap();

  drop(listening);

  assert_eq!(waiting.answer.await.unwrap(), AsideOutcome::Refused);
  assert!(aside::offer("aside-untaken", "third").is_none());
}
