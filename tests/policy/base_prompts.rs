use policy::base_prompts::*;

#[test]
fn the_rules_read_and_every_value_is_named_once() {
  assert!(BASE_PROMPTS.name_pattern.starts_with('^'));
  assert!(BASE_PROMPTS.values.contains(&"cwd".to_owned()));

  let mut names = BASE_PROMPTS.values.clone();

  names.sort();
  names.dedup();
  assert_eq!(names.len(), BASE_PROMPTS.values.len());
}
