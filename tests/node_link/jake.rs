//! A link to a person's PC (docs/person-nodes.md, 「The link over jake」): the folders and the bridge the far end
//! serves.

// "the folders the placed programs work in are made on the node, and one that stands is left as it is"
#[test]
fn the_folders_the_placed_programs_work_in_are_made_on_the_node() {
  let node = tempfile::tempdir().unwrap();
  let standing = node.path().join("data/supervisor-homes/kept");
  let wanted = node.path().join("data/supervisor-homes/new");

  std::fs::create_dir_all(&standing).unwrap();
  std::fs::write(standing.join("notes.md"), "kept").unwrap();

  node_link::serve::make_folders(&[standing.display().to_string(), wanted.display().to_string()]).unwrap();

  assert!(wanted.is_dir());
  assert_eq!(std::fs::read_to_string(standing.join("notes.md")).unwrap(), "kept");
}

// "a run on the node asks the home bridge and ends as the run at home ended, its output and errors its own"
#[tokio::test]
async fn a_run_on_the_node_asks_the_home_bridge_and_ends_as_the_run_at_home_ended() {
  use std::os::unix::fs::PermissionsExt;

  let dir = tempfile::tempdir().unwrap();
  let deploy = dir.path().join("deploy");
  let verbs = deploy.join("bin/bmo");

  std::fs::create_dir_all(verbs.parent().unwrap()).unwrap();
  std::fs::write(&verbs, "#!/bin/sh\necho \"$@\"\ncat\necho said >&2\nexit 5\n").unwrap();
  std::fs::set_permissions(&verbs, std::fs::Permissions::from_mode(0o755)).unwrap();

  let socket = dir.path().join("bridge.sock");
  let listener = tokio::net::UnixListener::bind(&socket).unwrap();

  tokio::spawn(async move {
    let (stream, _) = listener.accept().await.unwrap();

    node_link::bridge::answer(stream, deploy, "pc".into(), None, |path| path.replace("/n/data", "/h/data"))
      .await
      .unwrap();
  });

  let (mut out, mut err) = (Vec::new(), Vec::new());
  let args = ["agent-status", "status", "--place", "/n/data/agent-places/w"].map(str::to_owned);
  let code = node_link::bridge::ask(&socket, "bin/bmo", &args, &b"payload"[..], &mut out, &mut err).await;

  assert_eq!(code, 5);
  assert_eq!(
    String::from_utf8_lossy(&out),
    "agent-status status --place /h/data/agent-places/w\npayload"
  );
  assert_eq!(String::from_utf8_lossy(&err), "said\n");
}
