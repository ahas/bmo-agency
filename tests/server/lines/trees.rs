use agency_server::lines::*;

#[test]
fn a_mark_is_filled_where_it_is_named_and_left_where_it_is_not() {
  assert_eq!(render("a {{tools}} b {{other}}", &[("tools", "x")]), "a x b {{other}}");
}
