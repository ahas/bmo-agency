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

#[test]
fn every_seed_is_named_once_and_its_file_stands() {
  let mut names: Vec<&str> = BASE_PROMPTS.seed.iter().map(|seed| seed.name.as_str()).collect();

  names.sort_unstable();
  names.dedup();
  assert_eq!(names.len(), BASE_PROMPTS.seed.len());
  for seed in &BASE_PROMPTS.seed {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("prompts/base").join(&seed.file);

    assert!(file.is_file(), "{} names {}", seed.name, file.display());
    assert!(
      seed.name.len() <= BASE_PROMPTS.name_most && seed.label.chars().count() <= BASE_PROMPTS.label_most
    );
  }
}
