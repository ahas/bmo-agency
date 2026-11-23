//! The runner's keeping of personal apps (docs/person-nodes.md, 「Personal apps」): an app's folder of 'my apps'
//! placed as a new version, copied whole or built confined, its description answered; a version described and
//! pointed at, three older ones kept and the rest taken away; what each tool keeps listed; and a tool taken away, its
//! source and data left standing. A refusal names what failed and leaves nothing staged.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use serde_json::{Value, json};

/// A process the test started, killed and waited on as the test ends.
struct Started(Child);

impl Drop for Started {
  fn drop(&mut self) {
    let _ = self.0.kill();
    let _ = self.0.wait();
  }
}

// A stand-in for bubblewrap: the build after `--` run unconfined in the folder bound at /src, with `BMO_TOOL_OUT` the
// folder bound at /out, so the keeping is tried where no namespace can be made.
const FAKE_BWRAP: &str = r#"#!/usr/bin/env python3
import os, sys
args = sys.argv[1:]
source = out = None
for at, word in enumerate(args):
    if word == "--ro-bind" and args[at + 2] == "/src":
        source = args[at + 1]
    if word == "--bind" and args[at + 2] == "/out":
        out = args[at + 1]
    if word == "--":
        command = args[at + 1:]
        break
os.chdir(source)
os.environ["BMO_TOOL_OUT"] = out
os.execvp(command[0], command)
"#;

struct Machine {
  dir: tempfile::TempDir,
  bwrap: PathBuf,
}

impl Machine {
  fn new() -> Self {
    let dir = tempfile::tempdir().unwrap();
    let bwrap = dir.path().join("fake-bwrap");

    fs::write(&bwrap, FAKE_BWRAP).unwrap();
    fs::set_permissions(&bwrap, fs::Permissions::from_mode(0o755)).unwrap();
    Self { dir, bwrap }
  }

  fn root(&self) -> PathBuf {
    self.dir.path().join("data").join(node_link::runner::PERSONAL_TOOLS)
  }

  // An app's folder of 'my apps', its manifest naming `name`, with what `more` adds to its `[tool]` and a git folder.
  fn app(&self, folder: &str, name: &str, more: &str) -> PathBuf {
    let dir = self.dir.path().join("data").join(node_link::runner::MY_APPS).join(folder);

    fs::create_dir_all(dir.join(".git")).unwrap();
    fs::write(dir.join(".git").join("HEAD"), "ref: refs/heads/main").unwrap();
    fs::write(
      dir.join("tool.toml"),
      format!(
        "[tool]\nname = \"{name}\"\nprotocol = 1\nrun = [\"python3\", \"main.py\"]\n{more}\n[[entry]]\nnamespace = \"my\"\nname = \"{name}\"\ndescription = \"Sums up the week.\"\ninput = \"input.json\"\nmanual = \"docs/manual.md\"\ncapabilities = [\"my.own\"]\n"
      ),
    )
    .unwrap();
    fs::write(dir.join("input.json"), r#"{ "type": "object" }"#).unwrap();
    fs::create_dir_all(dir.join("docs")).unwrap();
    fs::write(dir.join("docs").join("manual.md"), "Call it with nothing.").unwrap();
    fs::write(dir.join("main.py"), "print('{}')").unwrap();
    dir
  }

  // One asking of the runner for `owner`, every line it answered.
  fn ask(&self, owner: &str, asked: Value) -> Vec<Value> {
    let mut command = Command::new(crate::support::binary::path("node-link"));

    command.args(["runner", "--root"]).arg(self.root()).args(["--owner", "a1"]);
    command.env(node_link::runner::BWRAP_VARIABLE, &self.bwrap);
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());

    let mut child = Started(command.spawn().expect("the runner"));
    let mut input = child.0.stdin.take().unwrap();
    let mut line = asked;

    line["owner"] = json!(owner);
    input.write_all(format!("{line}\n").as_bytes()).unwrap();

    let mut answered = Vec::new();
    let mut lines = BufReader::new(child.0.stdout.take().unwrap());

    loop {
      let mut read = String::new();

      if lines.read_line(&mut read).unwrap_or(0) == 0 {
        break;
      }

      let value: Value = serde_json::from_str(&read).unwrap();
      let last = value["t"] != "said";

      answered.push(value);

      if last {
        break;
      }
    }

    drop(input);
    answered
  }

  fn last(&self, asked: Value) -> Value {
    self.ask("a1", asked).pop().unwrap()
  }

  fn versions_dir(&self, tool: &str) -> PathBuf {
    self.root().join("owners").join("a1").join("tools").join(tool)
  }

  fn standing(&self, tool: &str) -> Vec<String> {
    let mut found: Vec<String> = fs::read_dir(self.versions_dir(tool))
      .into_iter()
      .flatten()
      .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
      .collect();

    found.sort();
    found
  }
}

fn listed(dir: &Path) -> Vec<String> {
  let mut found: Vec<String> = fs::read_dir(dir)
    .unwrap()
    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
    .collect();

  found.sort();
  found
}

// "an app's folder is placed whole as a new version, its .git left out, pointed at by nothing until it is pointed
// at, and its description answered; a point keeps three older versions and takes the rest away"
#[test]
fn a_place_copies_the_folder_into_a_version_and_a_point_keeps_three_older_ones() {
  let machine = Machine::new();

  machine.app("weekly", "weekly", "");

  let placed = machine.last(json!({ "place": { "tool": "weekly" } }));

  assert_eq!(placed["t"], "placed", "{placed}");

  let first = placed["version"].as_str().unwrap().to_owned();
  let files = placed["files"].as_object().unwrap();
  let mut paths: Vec<&String> = files.keys().collect();

  paths.sort();
  assert_eq!(paths, ["docs/manual.md", "input.json", "tool.toml"]);
  assert_eq!(files["docs/manual.md"], "Call it with nothing.");
  assert_eq!(
    listed(&machine.versions_dir("weekly").join(&first)),
    ["docs", "input.json", "main.py", "tool.toml"]
  );

  let kept = machine.last(json!({ "versions": {} }));

  assert_eq!(kept["tools"]["weekly"], json!({ "current": null, "kept": [first] }));

  let pointed = machine.last(json!({ "point": { "tool": "weekly", "version": first } }));

  assert_eq!(pointed, json!({ "t": "pointed", "version": first, "kept": [] }));

  let pointer: Value = serde_json::from_str(
    &fs::read_to_string(machine.root().join("owners/a1/registry/weekly.json")).unwrap(),
  )
  .unwrap();

  assert_eq!(pointer["version"], first);
  assert_eq!(pointer["owner"], "a1");

  // Four more versions, each pointed at: the newest three older ones are kept.
  let mut versions = vec![first.clone()];

  for _ in 0..4 {
    std::thread::sleep(std::time::Duration::from_millis(3));

    let placed = machine.last(json!({ "place": { "tool": "weekly" } }));
    let version = placed["version"].as_str().unwrap().to_owned();

    machine.last(json!({ "point": { "tool": "weekly", "version": version } }));
    versions.push(version);
  }

  let kept = machine.last(json!({ "versions": {} }));

  assert_eq!(kept["tools"]["weekly"]["current"], versions[4]);
  assert_eq!(kept["tools"]["weekly"]["kept"], json!([versions[3], versions[2], versions[1]]));
  assert_eq!(machine.standing("weekly"), versions[1..].to_vec(), "the oldest is taken away");

  // A kept version is described and pointed at again.
  let described = machine.last(json!({ "describe": { "tool": "weekly", "version": versions[2] } }));

  assert_eq!(described["t"], "described");
  assert_eq!(described["files"]["input.json"], r#"{ "type": "object" }"#);

  let back = machine.last(json!({ "point": { "tool": "weekly", "version": versions[2] } }));

  assert_eq!(back["kept"], json!([versions[1]]), "{back}");
  assert_eq!(
    machine.standing("weekly"),
    [versions[1].clone(), versions[2].clone()],
    "newer ones are taken away"
  );
}

// "a build runs with the app's folder read at /src and the new version written at /out, the manifest and the files
// it names copied beside what it wrote; a build that fails is refused with what it said, and nothing is staged"
#[test]
fn a_build_writes_the_version_and_a_failed_one_is_refused_with_its_lines() {
  let machine = Machine::new();

  machine.app("built", "built", "build = 'echo building >&2 && cp main.py \"$BMO_TOOL_OUT/run.py\"'");

  let answered = machine.ask("a1", json!({ "place": { "tool": "built" } }));
  let placed = answered.last().unwrap();

  assert_eq!(placed["t"], "placed", "{answered:?}");
  assert_eq!(answered[0], json!({ "t": "said", "line": "building" }));
  assert_eq!(
    listed(&machine.versions_dir("built").join(placed["version"].as_str().unwrap())),
    ["docs", "input.json", "run.py", "tool.toml"],
    "the source's own main.py stays out"
  );

  machine.app("broken", "broken", "build = 'echo it broke >&2; exit 3'");

  let refused = machine.last(json!({ "place": { "tool": "broken" } }));

  assert_eq!(refused["t"], "refused");
  assert!(refused["why"].as_str().unwrap().contains("build failed: it broke"), "{refused}");
  assert!(machine.standing("broken").is_empty(), "nothing is left staged");
}

// "a manifest not naming its folder, another account, a name that climbs and a version not placed are refused; a take
// leaves the source and the data standing"
#[test]
fn the_keeping_refuses_what_it_does_not_keep_and_a_take_leaves_source_and_data() {
  let machine = Machine::new();
  let source = machine.app("weekly", "monthly", "");
  let refused = |answered: Value| answered["why"].as_str().unwrap_or_default().to_owned();

  assert!(
    refused(machine.last(json!({ "place": { "tool": "weekly" } }))).contains("not its folder's name")
  );
  assert!(
    refused(machine.ask("b2", json!({ "versions": {} })).pop().unwrap())
      .contains("keeps no app of the account b2")
  );
  assert!(refused(machine.last(json!({ "place": { "tool": "../x" } }))).contains("is no tool's name"));
  assert!(
    refused(machine.last(json!({ "point": { "tool": "weekly", "version": "v9" } })))
      .contains("is not placed")
  );

  machine.app("weekly", "weekly", "");

  let placed = machine.last(json!({ "place": { "tool": "weekly" } }));

  machine.last(json!({ "point": { "tool": "weekly", "version": placed["version"] } }));

  let data = node_link::runner::data_dir(&machine.root(), "a1", "weekly");

  fs::create_dir_all(&data).unwrap();
  fs::write(data.join("notes.txt"), "kept").unwrap();

  assert_eq!(machine.last(json!({ "take": { "tool": "weekly" } })), json!({ "t": "taken" }));
  assert!(!machine.versions_dir("weekly").exists());
  assert!(!machine.root().join("owners/a1/registry/weekly.json").exists());
  assert!(source.join("tool.toml").exists(), "the source stands");
  assert_eq!(fs::read_to_string(data.join("notes.txt")).unwrap(), "kept", "the data stands");
  assert!(refused(machine.last(json!({ "take": { "tool": "weekly" } }))).contains("keeps no app weekly"));
}
