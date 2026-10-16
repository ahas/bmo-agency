use policy::chat_line::*;

#[test]
fn reads_its_lines() {
  assert!(CHAT_LINE.tools.allowed.starts_with(&CHAT_LINE.tools.shown));
  assert!(CHAT_LINE.room.mark.contains("{title}"));
  assert_eq!(CHAT_LINE.persona.default_tone, "bmo");
}
