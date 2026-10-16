use policy::tiers::*;

#[test]
fn the_coworker_tiers_stand_on_the_ladder_in_its_order() {
  let on_ladder: Vec<&str> =
    TIERS.ladder.iter().map(String::as_str).filter(|tier| TIERS.lends(tier)).collect();

  assert_eq!(on_ladder, TIERS.coworker_tiers());
  assert!(!TIERS.lends("BMO") && !TIERS.lends("Frontier"));
}
