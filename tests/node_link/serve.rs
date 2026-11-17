//! The far end on the node: what it watches, sends and writes.

use std::fs;

use node_link::frame::{Identity, Pull};
use node_link::serve::*;

#[test]
fn a_tree_names_every_file_under_it() {
  let dir = tempfile::tempdir().expect("dir");

  fs::create_dir_all(dir.path().join("a/b")).expect("tree");
  fs::write(dir.path().join("a/b/one.jsonl"), "x").expect("one");
  fs::write(dir.path().join("two.jsonl"), "y").expect("two");

  let mut tree = files_of(&Pull {
    remote: dir.path().to_string_lossy().into(),
    files: vec![],
    tree: true,
    append: true,
    alive: false,
  });
  let flat = files_of(&Pull {
    remote: dir.path().to_string_lossy().into(),
    files: vec![],
    tree: false,
    append: true,
    alive: false,
  });

  tree.sort();
  assert_eq!(tree, vec!["a/b/one.jsonl".to_owned(), "two.jsonl".to_owned()]);
  assert_eq!(flat, vec!["two.jsonl".to_owned()]);
}

#[test]
fn only_a_live_process_of_the_record_is_told_alive() {
  let dir = tempfile::tempdir().expect("dir");
  let own = std::process::id() as i32;
  let birth = process_birth(own).expect("own birth");

  fs::write(
    dir.path().join("running.json"),
    serde_json::json!({ "pid": own, "birth": birth, "loop": { "pid": own, "birth": "0" } }).to_string(),
  )
  .expect("record");

  assert_eq!(alive_in(dir.path()), vec![Identity { pid: own, birth }]);
}

// "a newer mirror session ends the one before it, and is ended in turn by the next"
#[test]
fn a_newer_mirror_session_ends_the_one_before_it_and_is_ended_in_turn_by_the_next() {
  let mirror = Mirror::default();
  let mut first = mirror.newest();

  assert!(first.try_recv().is_err(), "a session alone is not told to end");

  let mut second = mirror.newest();

  assert_eq!(first.try_recv(), Ok(()));
  assert!(second.try_recv().is_err());

  let _third = mirror.newest();

  assert_eq!(second.try_recv(), Ok(()));
}

// "on a person's PC a near end that connects closes the mirror an earlier near end left standing"
#[tokio::test]
async fn on_a_person_s_pc_a_near_end_that_connects_closes_the_mirror_an_earlier_near_end_left_standing() {
  use node_link::frame::Frame;
  use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

  let root = tempfile::tempdir().unwrap();
  let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
  let serving = tokio::spawn(serve(Serve {
    root: root.path().to_path_buf(),
    bridge: None,
    herdr_session: None,
    herdr_socket: None,
    listen: Some(format!("127.0.0.1:{port}")),
    herdr_listen: None,
    herdr_unix: None,
  }));
  // A near end that is greeted and opens a mirror session, as a link does.
  let near_end = || async move {
    let stream = loop {
      match tokio::net::TcpStream::connect(("127.0.0.1", port)).await {
        Ok(stream) => break stream,
        Err(_) => tokio::task::yield_now().await,
      }
    };
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut hello = String::new();

    reader.read_line(&mut hello).await.unwrap();
    assert!(hello.contains("hello"), "{hello}");
    writer.write_all(Frame::Carry { ports: vec![] }.line().as_bytes()).await.unwrap();
    (reader, writer)
  };

  let (mut left, _left_open) = near_end().await;
  let (_now, _now_open) = near_end().await;
  let mut rest = Vec::new();
  let closed = tokio::time::timeout(
    std::time::Duration::from_secs(10),
    tokio::io::AsyncReadExt::read_to_end(&mut left, &mut rest),
  )
  .await;

  assert!(matches!(closed, Ok(Ok(_))), "the earlier near end's mirror is closed: {closed:?}");

  serving.abort();
}
