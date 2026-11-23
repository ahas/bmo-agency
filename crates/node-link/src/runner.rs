//! The runner of a person's personal tools (docs/person-nodes.md, 「Personal tools on a PC」): `node-link runner`,
//! started for one call, speaking the tool host's runner passage on its standard input and output (bmo-pocket's
//! docs/api.md, 「The runner」). It runs the version of the owner's tool the call names, confined by bubblewrap to
//! that version's folder, read alone, and the tool's own data folder, with a clean environment and no network, and
//! answers what the tool wrote and how it ended. Its standard input closing before the tool has answered is its
//! caller gone, and ends the tool.

use std::collections::BTreeMap;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

/// The folder under a workplace's data root a PC's runner keeps its owners' tools in.
pub const PERSONAL_TOOLS: &str = "personal-tools";

/// The folder beside it, under the same data root, the person's apps' sources stand in, one folder an app.
pub const MY_APPS: &str = "my-apps";

/// The folder of agency's tree the screen kit stands in (docs/person-nodes.md, 「Personal screens」).
pub const SCREEN_KIT: &str = "screen-kit";

/// The variable a test names a stand-in for bubblewrap in.
pub const BWRAP_VARIABLE: &str = "NODE_LINK_BWRAP";

/// Where a confined tool sees its version folder and its data folder.
const TOOL_AT: &str = "/tool";
const DATA_AT: &str = "/data";

/// The search path a confined tool runs with: the machine's own programs, read alone.
const CONFINED_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// The machine's own folders a confined tool reads, where they stand as folders; where one is a link, as `/bin` is
/// to `usr/bin` on most systems, the link is made again inside.
const SYSTEM_FOLDERS: [&str; 5] = ["/bin", "/sbin", "/lib", "/lib32", "/lib64"];

/// The machine's own files a confined tool reads where they stand: what the dynamic loader and the clock read, and
/// the alternatives a program of `/usr` is often named through.
const SYSTEM_FILES: [&str; 5] = [
  "/etc/ld.so.cache",
  "/etc/ld.so.conf",
  "/etc/ld.so.conf.d",
  "/etc/localtime",
  "/etc/alternatives",
];

/// What the runner is asked on the passage's first line (bmo-pocket's docs/api.md, 「The runner」).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asked {
  pub owner: String,
  pub tool: String,
  pub version: String,
  pub run: Vec<String>,
  pub call: Value,
}

/// One line the runner answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "kebab-case")]
pub enum Answered {
  /// A line the tool wrote on its standard error.
  Said { line: String },
  /// How the tool ended, none where a signal ended it, and what it wrote on its standard output.
  End { code: Option<i32>, out: String },
  /// Why the runner did not run the tool, or did not do what it was asked.
  Refused { why: String },
  /// An app placed as a new version, pointed at by nothing yet, and its description by path.
  Placed {
    version: String,
    files: BTreeMap<String, String>,
  },
  /// A version's description by path.
  Described {
    version: String,
    files: BTreeMap<String, String>,
  },
  /// The pointer moved onto the version, and the versions kept older than it, newest first.
  Pointed { version: String, kept: Vec<String> },
  /// Each tool's version pointed at and those kept beside it, by the tool's name.
  Versions { tools: BTreeMap<String, Kept> },
  /// The tool's pointer and versions taken away.
  Taken {},
  /// One file of a version's screen, its bytes in base64.
  File { bytes: String },
  /// The source a version was placed from, by path.
  Source {
    version: String,
    files: BTreeMap<String, String>,
  },
  /// A shared app's source written into its folder of 'my apps'.
  Received {},
  /// A received app made the person's own.
  Adopted {},
}

/// What a tool keeps: the version pointed at, none where no pointer stands, and the others older than it, newest
/// first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
  pub current: Option<String>,
  pub kept: Vec<String>,
}

impl Answered {
  pub fn line(&self) -> String {
    let mut line = serde_json::to_string(self).unwrap_or_default();

    line.push('\n');
    line
  }
}

/// How a runner is started: the folder its owners' tools stand in, the one owner it runs for where it is told one,
/// the bubblewrap program it confines with, and the screen kit a place's build sees, where it is told one.
#[derive(Clone, Debug)]
pub struct Runner {
  pub root: PathBuf,
  pub owner: Option<String>,
  pub bwrap: String,
  pub kit: Option<PathBuf>,
}

/// A name a folder of the runner's holds: an owner's, a tool's or a version's, never one that climbs or hides.
pub fn plain_name(name: &str) -> bool {
  !name.is_empty()
    && !name.starts_with('.')
    && name.chars().all(|one| one.is_ascii_alphanumeric() || matches!(one, '-' | '_' | '.'))
}

/// The folder an owner's placed version of a tool stands in, which the tool reads alone.
pub fn version_dir(root: &Path, owner: &str, tool: &str, version: &str) -> PathBuf {
  root.join("owners").join(owner).join("tools").join(tool).join(version)
}

/// The folder the apps' sources stand in, one folder an app: the runner's root's sibling `my-apps`, under the same
/// data root (docs/person-nodes.md, 「Personal apps」).
pub fn apps_dir(root: &Path) -> PathBuf {
  root.parent().map_or_else(|| PathBuf::from(MY_APPS), |parent| parent.join(MY_APPS))
}

/// The folder an owner's tool keeps its data in, the one it writes.
pub fn data_dir(root: &Path, owner: &str, tool: &str) -> PathBuf {
  root.join("owners").join(owner).join("data").join(tool)
}

/// The words bubblewrap is started with to run `run` confined: every namespace its own, the network among them;
/// ended with its parent; in a session of its own; a clean environment; the machine's `/usr` and the few files its
/// programs read, read alone; a process table, devices and a scratch folder of its own; the version folder read alone
/// at `/tool`, where it starts, and the data folder written at `/data`, its home. The first run word is read in the
/// version folder where a file of that name stands there, and found on the search path otherwise.
pub fn confined(version: &Path, data: &Path, run: &[String]) -> Vec<String> {
  let mut words = machine_words();

  words.extend(["--ro-bind".into(), version.to_string_lossy().into_owned(), TOOL_AT.into()]);
  words.extend(["--bind".into(), data.to_string_lossy().into_owned(), DATA_AT.into()]);
  words.extend(["--chdir".into(), TOOL_AT.into()]);

  for (name, value) in [("PATH", CONFINED_PATH), ("HOME", DATA_AT), ("TMPDIR", "/tmp"), ("LANG", "C.UTF-8")]
  {
    words.extend(["--setenv".into(), name.into(), value.into()]);
  }

  words.push("--".into());

  if let Some((first, rest)) = run.split_first() {
    let own = version.join(first);

    words.push(if plain_name(first) && own.is_file() {
      format!("{TOOL_AT}/{first}")
    } else {
      first.clone()
    });
    words.extend(rest.iter().cloned());
  }

  words
}

/// The words every confinement starts with: every namespace its own, the network among them; ended with its
/// parent; in a session of its own; a clean environment; the machine's `/usr` and the few files its programs read,
/// read alone; and a process table, devices and a scratch folder of its own.
pub fn machine_words() -> Vec<String> {
  let mut words: Vec<String> = [
    "--unshare-all",
    "--die-with-parent",
    "--new-session",
    "--clearenv",
    "--ro-bind",
    "/usr",
    "/usr",
  ]
  .map(str::to_owned)
  .to_vec();

  for folder in SYSTEM_FOLDERS {
    let path = Path::new(folder);

    match std::fs::read_link(path) {
      Ok(target) => {
        words.extend(["--symlink".into(), target.to_string_lossy().into_owned(), folder.into()])
      }
      Err(_) if path.is_dir() => words.extend(["--ro-bind".into(), folder.into(), folder.into()]),
      Err(_) => {}
    }
  }

  for file in SYSTEM_FILES {
    words.extend(["--ro-bind-try".into(), file.into(), file.into()]);
  }

  words.extend(["--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp"].map(str::to_owned));
  words
}

/// The search path a confined program runs with.
pub fn confined_path() -> &'static str {
  CONFINED_PATH
}

/// Where a program named on the search path stands, its links followed; none where it is not found.
pub fn program_path(program: &str) -> Option<PathBuf> {
  std::env::var_os("PATH")
    .and_then(|path| {
      std::env::split_paths(&path).map(|dir| dir.join(program)).find(|found| found.is_file())
    })
    .and_then(|found| std::fs::canonicalize(found).ok())
}

/// Whether a program stands where it is named: a path that is a file, or a name found on the search path.
pub fn program_stands(program: &str) -> bool {
  if program.contains('/') {
    return Path::new(program).is_file();
  }

  std::env::var_os("PATH")
    .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

/// What the runner is asked, held to what it runs: an owner it runs for, plain names, a version that stands and a
/// run command; the version and data folders, the data folder made where it is not.
fn placed(runner: &Runner, asked: &Asked) -> Result<(PathBuf, PathBuf), String> {
  if runner.owner.as_deref().is_some_and(|owner| owner != asked.owner) {
    return Err(format!("this runner runs no personal tool of the account {}", asked.owner));
  }

  for (what, name) in [("owner", &asked.owner), ("tool", &asked.tool), ("version", &asked.version)] {
    if !plain_name(name) {
      return Err(format!("{name:?} is no {what}'s name"));
    }
  }

  if asked.run.first().is_none_or(|word| word.trim().is_empty()) {
    return Err("the call names no run command".to_owned());
  }

  let version = version_dir(&runner.root, &asked.owner, &asked.tool, &asked.version);

  if !version.is_dir() {
    return Err(format!(
      "version {} of the tool {} is not placed on this machine for the account {}",
      asked.version, asked.tool, asked.owner
    ));
  }

  let data = data_dir(&runner.root, &asked.owner, &asked.tool);

  std::fs::DirBuilder::new()
    .recursive(true)
    .mode(0o700)
    .create(&data)
    .map_err(|why| format!("{}: {why}", data.display()))?;

  if !program_stands(&runner.bwrap) {
    return Err(format!(
      "bubblewrap ({}) is not on this machine, and a personal tool runs confined or not at all",
      runner.bwrap
    ));
  }

  Ok((version, data))
}

/// One call run: its first line read from `input`, the tool run confined, each line it writes on its errors and its
/// end answered on `output`. `input` closing before the tool ends is its caller gone: the tool's process is ended
/// and nothing more is answered.
pub async fn run(
  runner: Runner,
  input: impl AsyncRead + Unpin + Send + 'static,
  mut output: impl AsyncWrite + Unpin + Send + 'static,
) {
  let mut input = BufReader::new(input);
  let mut first = String::new();

  if !matches!(input.read_line(&mut first).await, Ok(read) if read > 0) {
    return;
  }

  // A line naming no call asks the runner to keep the owner's versions (docs/person-nodes.md, 「Personal apps」).
  if serde_json::from_str::<Value>(first.trim_end()).is_ok_and(|line| line.get("call").is_none()) {
    crate::keeping::keep(runner, first.trim_end(), input, output).await;
    return;
  }

  let asked = match serde_json::from_str::<Asked>(first.trim_end()) {
    Ok(asked) => asked,
    Err(why) => {
      let refused = Answered::Refused {
        why: format!("the call does not read: {why}"),
      };

      let _ = output.write_all(refused.line().as_bytes()).await;
      return;
    }
  };
  let (version, data) = match placed(&runner, &asked) {
    Ok(folders) => folders,
    Err(why) => {
      let _ = output.write_all(Answered::Refused { why }.line().as_bytes()).await;
      return;
    }
  };
  let mut child = match tokio::process::Command::new(&runner.bwrap)
    .args(confined(&version, &data, &asked.run))
    .env_clear()
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true)
    .spawn()
  {
    Ok(child) => child,
    Err(why) => {
      let refused = Answered::Refused {
        why: format!("bubblewrap ({}) could not be started: {why}", runner.bwrap),
      };

      let _ = output.write_all(refused.line().as_bytes()).await;
      return;
    }
  };
  let call = serde_json::to_vec(&asked.call).unwrap_or_default();

  if let Some(mut stdin) = child.stdin.take() {
    tokio::spawn(async move {
      let _ = stdin.write_all(&call).await;
    });
  }

  let (lines, mut answering) = mpsc::unbounded_channel::<Answered>();
  let writer = tokio::spawn(async move {
    while let Some(answered) = answering.recv().await {
      if output.write_all(answered.line().as_bytes()).await.is_err() || output.flush().await.is_err() {
        break;
      }
    }
  });
  let said = child.stderr.take().map(|stderr| {
    let lines = lines.clone();

    tokio::spawn(async move {
      let mut read = BufReader::new(stderr).lines();

      while let Ok(Some(line)) = read.next_line().await {
        let _ = lines.send(Answered::Said { line });
      }
    })
  });
  let mut stdout = child.stdout.take();
  let reading = async move {
    let mut out = Vec::new();

    if let Some(stdout) = stdout.as_mut() {
      let _ = stdout.read_to_end(&mut out).await;
    }

    out
  };
  // Nothing more comes on the passage after its first line, so its end is the caller gone.
  let gone = async move {
    let mut rest = Vec::new();
    let _ = input.read_to_end(&mut rest).await;
  };

  tokio::select! {
    out = reading => {
      let ended = child.wait().await.ok();

      if let Some(said) = said {
        let _ = said.await;
      }

      let _ = lines.send(Answered::End {
        code: ended.and_then(|status| status.code()),
        out: String::from_utf8_lossy(&out).into_owned(),
      });
    }
    () = gone => {
      let _ = child.start_kill();
      let _ = child.wait().await;
    }
  }

  drop(lines);
  let _ = writer.await;
}
