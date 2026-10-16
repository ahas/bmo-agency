//! A conversational agent's settings and prompt folder as its row holds them.

use agency_server::conversational::*;

use crate::scratch::Scratch;

#[test]
fn a_row_with_settings_is_read_by_them_whatever_its_kind() {
  let read = Settings::read("companion", Some("agent"), Some("window"), 1).unwrap();

  assert_eq!(read.scope, Scope::Agent);
  assert_eq!(read.shape, Shape::Window);
  assert!(read.default_answerer && !read.companion());
  assert_eq!(Settings::read("chat", Some("room"), None, 0).unwrap().shape, Shape::Pieces);
}

#[test]
fn a_row_an_older_writer_left_is_read_by_its_kind() {
  assert_eq!(Settings::read("companion", None, None, 0), Some(Settings::companion_added()));
  assert!(Settings::read("chat", None, None, 0).unwrap().default_answerer);
  assert_eq!(Settings::read("worker", None, None, 0), None);
  assert_eq!(Settings::read("worker", Some("elsewhere"), None, 0), None);
}

#[tokio::test]
async fn a_companion_s_prompt_folder_is_the_one_its_row_names_and_none_otherwise() {
  let scratch = Scratch::new().await;

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "prompts")
         VALUES ('pal', 'companion', 'pal-prompts'), ('added', 'companion', NULL)"#,
      vec![],
    )
    .await;

  assert_eq!(prompt_folder(scratch.db(), "pal").await.unwrap().as_deref(), Some("pal-prompts"));
  assert_eq!(prompt_folder(scratch.db(), "added").await.unwrap(), None);
  assert_eq!(prompt_folder(scratch.db(), "nobody").await.unwrap(), None);
}
