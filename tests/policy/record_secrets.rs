use policy::record_secrets::RECORD_SECRETS;

#[test]
fn the_record_secrets_read_with_a_path_under_the_home_and_the_words_a_secret_is_named_by() {
  assert!(RECORD_SECRETS.receiver_shapes.ends_with("policy/secret-shapes.toml"));
  assert!(!RECORD_SECRETS.receiver_shapes.starts_with('/'));
  assert!(RECORD_SECRETS.secret_words.iter().any(|word| word == "token"));
  assert!(RECORD_SECRETS.least_characters > 0);
}
