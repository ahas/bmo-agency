//! The hooks: a point answers its default until a consumer registers there, and again once it is let
//! go.

use agency_server::hooks::{HookPoint, Hooks};

struct Pick;

impl HookPoint for Pick {
  type Ask = u32;
  type Answer = String;
  const NAME: &'static str = "pick";

  fn default_answer(ask: u32) -> String {
    format!("default for {ask}")
  }
}

struct Other;

impl HookPoint for Other {
  type Ask = ();
  type Answer = bool;
  const NAME: &'static str = "other";

  fn default_answer(_ask: ()) -> bool {
    false
  }
}

#[tokio::test]
async fn a_point_nobody_registered_at_answers_its_default() {
  let hooks = Hooks::default();

  assert!(!hooks.registered::<Pick>());
  assert_eq!(hooks.ask::<Pick>(3).await, "default for 3");
}

#[tokio::test]
async fn a_registered_answer_is_asked_until_it_is_let_go() {
  let hooks = Hooks::default();

  hooks.clone().register::<Pick, _, _>(|ask| async move { format!("consumer's {ask}") });

  assert!(hooks.registered::<Pick>());
  assert_eq!(hooks.ask::<Pick>(4).await, "consumer's 4");
  assert!(!hooks.ask::<Other>(()).await, "another point keeps its default");

  hooks.register::<Pick, _, _>(|ask| async move { format!("replaced {ask}") });

  assert_eq!(hooks.ask::<Pick>(5).await, "replaced 5");

  hooks.unregister::<Pick>();

  assert_eq!(hooks.ask::<Pick>(6).await, "default for 6");
}
