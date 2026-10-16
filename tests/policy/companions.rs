use policy::companions::*;

#[test]
fn the_pieces_read_in_their_default_order() {
  assert_eq!(COMPANIONS.pieces.last().map(String::as_str), Some("persona"));
  assert!(COMPANIONS.is_piece("frame") && !COMPANIONS.is_piece("greeting"));
}

#[test]
fn only_the_answer_ceiling_is_held_by_the_window() {
  let open: Vec<&str> = GENERATION
    .settings
    .iter()
    .filter(|one| one.bounds.as_ref().is_some_and(|bounds| bounds.max.is_none()))
    .map(|one| one.name.as_str())
    .collect();

  assert_eq!(open, ["max_tokens"]);
  assert!(
    GENERATION
      .setting("top_p")
      .is_some_and(|one| one.bounds.as_ref().is_some_and(|bounds| bounds.above_min))
  );
}

#[test]
fn a_switch_has_no_bounds_and_every_other_setting_has_them() {
  for one in &GENERATION.settings {
    assert_eq!(one.switch, one.bounds.is_none(), "{}", one.name);
  }

  assert!(GENERATION.setting("preserve_thinking").is_some_and(|one| one.switch && one.template));
}
