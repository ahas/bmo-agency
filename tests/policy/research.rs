use policy::research::*;

#[test]
fn a_setting_opens_its_capability_where_it_names_something_and_no_setting_opens_none() {
  let set = |name: &str| match name {
    "BMO_RESEARCH_IMAGES" => Some("1"),
    "BMO_RESEARCH_TOOLS" => Some("0"),
    "BMO_RESEARCH_RULEBOOKS" => Some("a,,b"),
    _ => None,
  };

  assert_eq!(RESEARCH.withheld(true, set), ["web.branches"]);
  assert_eq!(RESEARCH.withheld(false, set), ["web.branches", "web.images", "web.rulebooks"]);
  assert!(!opens("") && !opens(",0,") && opens("0,goods"));
}

#[test]
fn a_search_is_bound_by_every_rulebook_it_may_be_given_on_request() {
  assert!(RESEARCH.pool_capacity_most > 0);
  assert!(!RESEARCH.search.applies_section.is_empty());
  assert!(
    RESEARCH
      .search
      .on_request
      .iter()
      .all(|rulebook| RESEARCH.search.rulebooks.contains(rulebook))
  );
}

#[test]
fn every_research_line_ends_in_the_final_question_and_holds_it_nowhere_else() {
  assert!(!RESEARCH.draft.lines.is_empty());

  for (line, rounds) in &RESEARCH.draft.lines {
    assert!(
      rounds.rounds.last().is_some_and(|name| name == Draft::FINAL),
      "{line} does not end in the final question"
    );
    assert_eq!(
      rounds.rounds.iter().filter(|name| name.as_str() == Draft::FINAL).count(),
      1,
      "{line} holds the final question more than once"
    );
    assert!(rounds.ceiling_seconds > 0, "{line} has no ceiling");
    assert!(
      rounds.rounds.iter().all(|name| RESEARCH.draft.question(name).is_some()),
      "{line} names a question the catalogue does not hold"
    );
  }
}
