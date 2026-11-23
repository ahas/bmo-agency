//! The runner's keeping of an owner's personal apps (docs/person-nodes.md, 「Personal apps」): an app's folder of
//! 'my apps' built confined into a new version, a version's description read, the tool's pointer moved and the
//! versions beyond those kept taken away, what each tool keeps listed, and a tool taken away. Each is asked on the
//! passage's first line, `{"owner": <account>, "<what>": {…}}`, and answered by one line.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Deserialize;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

use crate::runner::{Answered, Kept, Runner, apps_dir, machine_words, plain_name, program_stands};

/// The manifest's file in an app's folder and in each version.
pub const MANIFEST: &str = "tool.toml";

/// How many versions older than the one pointed at a tool keeps.
pub const KEEP: usize = 3;

/// Where a confined build reads the app's folder and writes the new version.
const SOURCE_AT: &str = "/src";
const OUT_AT: &str = "/out";

/// What a keeping asks, by the one field beside `owner` its line names.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum Asked {
  Place { tool: String },
  Describe { tool: String, version: String },
  Point { tool: String, version: String },
  Versions {},
  Take { tool: String },
}

#[derive(Deserialize)]
struct Line {
  owner: String,
  #[serde(flatten)]
  asked: Asked,
}

// What of a manifest the runner reads: the tool's name and build, and the files its entries name. The tool host
// reads the whole of it when the description is put at home.
#[derive(Deserialize)]
struct Manifest {
  tool: ToolPart,
  #[serde(default)]
  entry: Vec<EntryPart>,
}

#[derive(Deserialize)]
struct ToolPart {
  name: String,
  #[serde(default)]
  build: Option<String>,
}

#[derive(Deserialize)]
struct EntryPart {
  input: String,
  #[serde(default)]
  output: Option<String>,
  #[serde(default)]
  manual: Option<String>,
}

impl Manifest {
  fn read(folder: &Path) -> Result<Self, String> {
    let text = std::fs::read_to_string(folder.join(MANIFEST))
      .map_err(|why| format!("{} holds no {MANIFEST}: {why}", folder.display()))?;

    toml::from_str(&text).map_err(|why| format!("its {MANIFEST} does not read: {why}"))
  }

  // The manifest and every file its entries name, each by its path in the folder.
  fn named_files(&self) -> Vec<String> {
    let mut files = vec![MANIFEST.to_owned()];

    for entry in &self.entry {
      files.extend(
        std::iter::once(entry.input.clone())
          .chain(entry.output.clone())
          .chain(entry.manual.clone()),
      );
    }

    files.sort();
    files.dedup();
    files
  }
}

// A path inside a folder, made of folder and file names alone, so it never climbs out.
fn inside(path: &str) -> bool {
  !path.is_empty()
    && Path::new(path).components().all(|part| matches!(part, std::path::Component::Normal(_)))
}

/// The folders of one owner's tool: its pointer, its versions and its app's source.
struct Owned {
  root: PathBuf,
  apps: PathBuf,
  owner: String,
}

impl Owned {
  fn of(runner: &Runner, owner: &str) -> Self {
    Self {
      root: runner.root.join("owners").join(owner),
      apps: apps_dir(&runner.root),
      owner: owner.to_owned(),
    }
  }

  fn versions(&self, tool: &str) -> PathBuf {
    self.root.join("tools").join(tool)
  }

  fn pointer(&self, tool: &str) -> PathBuf {
    self.root.join("registry").join(format!("{tool}.json"))
  }

  fn current(&self, tool: &str) -> Option<String> {
    let text = std::fs::read_to_string(self.pointer(tool)).ok()?;
    let pointer: serde_json::Value = serde_json::from_str(&text).ok()?;

    pointer["version"].as_str().map(str::to_owned)
  }

  // The versions standing of a tool, staged ones left out, oldest first: a version's name opens on the moment it
  // was placed.
  fn standing(&self, tool: &str) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(self.versions(tool))
      .into_iter()
      .flatten()
      .flatten()
      .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
      .filter_map(|entry| entry.file_name().into_string().ok())
      .filter(|name| plain_name(name))
      .collect();

    found.sort_by(|one, other| (one.len(), one).cmp(&(other.len(), other)));
    found
  }

  // What a tool keeps: its version pointed at and those older than it, newest first; every version where no
  // pointer stands.
  fn kept(&self, tool: &str) -> Kept {
    let current = self.current(tool);
    let mut older: Vec<String> = self
      .standing(tool)
      .into_iter()
      .take_while(|version| Some(version) != current.as_ref())
      .collect();

    older.reverse();
    Kept {
      current,
      kept: older,
    }
  }
}

// A version's name: the moment it is placed, and a suffix no other place of the same moment holds.
fn fresh() -> String {
  let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
  let suffix = (now.subsec_nanos() ^ std::process::id().rotate_left(16)) & 0x00ff_ffff;

  format!("{}-{suffix:06x}", now.as_millis())
}

// A folder copied whole into another, its `.git` left out.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
  std::fs::create_dir_all(to)?;

  for entry in std::fs::read_dir(from)? {
    let entry = entry?;

    if entry.file_name() == ".git" {
      continue;
    }

    let (path, into) = (entry.path(), to.join(entry.file_name()));
    let kind = entry.file_type()?;

    if kind.is_dir() {
      copy_tree(&path, &into)?;
    } else if kind.is_file() {
      std::fs::copy(&path, &into)?;
    }
  }

  Ok(())
}

// A version's description: its manifest and the files it names, as text, by path.
fn description(version: &Path) -> Result<BTreeMap<String, String>, String> {
  let manifest = Manifest::read(version)?;
  let mut files = BTreeMap::new();

  for file in manifest.named_files() {
    if !inside(&file) {
      return Err(format!("its manifest names {file}, which is not inside its folder"));
    }

    let text = std::fs::read_to_string(version.join(&file)).map_err(|why| format!("{file}: {why}"))?;

    files.insert(file, text);
  }

  Ok(files)
}

/// The words bubblewrap runs a build with: the machine's own confinement, the app's folder read alone at `/src`,
/// where it starts, and the new version's folder written at `/out`, which `BMO_TOOL_OUT` names; `sh` running the
/// manifest's build.
pub fn build_words(source: &Path, out: &Path, build: &str) -> Vec<String> {
  let mut words = machine_words();

  words.extend(["--ro-bind".into(), source.to_string_lossy().into_owned(), SOURCE_AT.into()]);
  words.extend(["--bind".into(), out.to_string_lossy().into_owned(), OUT_AT.into()]);
  words.extend(["--chdir".into(), SOURCE_AT.into()]);

  for (name, value) in [
    ("PATH", crate::runner::confined_path()),
    ("HOME", "/tmp"),
    ("TMPDIR", "/tmp"),
    ("LANG", "C.UTF-8"),
    ("BMO_TOOL_OUT", OUT_AT),
  ] {
    words.extend(["--setenv".into(), name.into(), value.into()]);
  }

  words.extend(["--".into(), "sh".into(), "-c".into(), build.to_owned()]);
  words
}

// The next line a build wrote on its errors, none once they end.
async fn next_line(
  errors: &mut Option<tokio::io::Lines<BufReader<tokio::process::ChildStderr>>>,
) -> Option<String> {
  match errors {
    Some(lines) => lines.next_line().await.ok().flatten(),
    None => None,
  }
}

// A staged folder taken away as it is dropped, unless it was kept.
struct Staged(Option<PathBuf>);

impl Drop for Staged {
  fn drop(&mut self) {
    if let Some(dir) = &self.0 {
      let _ = std::fs::remove_dir_all(dir);
    }
  }
}

// The app's folder placed as a new version: built confined where its manifest names a build, copied whole
// otherwise; each line the build writes on its errors said as it comes. The caller going away ends the build.
async fn place(
  runner: &Runner,
  owned: &Owned,
  tool: &str,
  input: &mut (impl AsyncRead + Unpin),
  said: &mut (impl AsyncWrite + Unpin),
) -> Answered {
  let source = owned.apps.join(tool);
  let manifest = match Manifest::read(&source) {
    Ok(manifest) => manifest,
    Err(why) => {
      return Answered::Refused {
        why: format!("the app {tool}: {why}"),
      };
    }
  };

  if manifest.tool.name != tool {
    return Answered::Refused {
      why: format!(
        "the app {tool}'s manifest names the tool {}, not its folder's name",
        manifest.tool.name
      ),
    };
  }

  let version = fresh();
  let staged = owned.versions(tool).join(format!(".staging-{version}"));
  let mut staging = Staged(Some(staged.clone()));

  if let Err(why) = std::fs::create_dir_all(&staged) {
    return Answered::Refused {
      why: format!("{}: {why}", staged.display()),
    };
  }

  match manifest.tool.build.as_deref().filter(|build| !build.trim().is_empty()) {
    Some(build) => {
      if !program_stands(&runner.bwrap) {
        return Answered::Refused {
          why: format!(
            "bubblewrap ({}) is not on this machine, and an app builds confined or not at all",
            runner.bwrap
          ),
        };
      }

      let built = tokio::process::Command::new(&runner.bwrap)
        .args(build_words(&source, &staged, build))
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn();
      let mut child = match built {
        Ok(child) => child,
        Err(why) => {
          return Answered::Refused {
            why: format!("bubblewrap ({}) could not be started: {why}", runner.bwrap),
          };
        }
      };
      let mut errors = child.stderr.take().map(|stderr| BufReader::new(stderr).lines());
      let mut last = Vec::new();
      let mut rest = Vec::new();
      let gone = input.read_to_end(&mut rest);

      tokio::pin!(gone);

      loop {
        tokio::select! {
          line = next_line(&mut errors) => match line {
            Some(line) => {
              let _ = said.write_all(Answered::Said { line: line.clone() }.line().as_bytes()).await;
              last.push(line);

              if last.len() > 5 {
                last.remove(0);
              }
            }
            None => break,
          },
          _ = &mut gone => {
            let _ = child.start_kill();
            let _ = child.wait().await;

            return Answered::Refused { why: "the caller went away before the build ended".to_owned() };
          }
        }
      }

      let ended = child.wait().await.ok();

      if !ended.is_some_and(|status| status.success()) {
        return Answered::Refused {
          why: format!("the app {tool}'s build failed: {}", last.join("\n")),
        };
      }

      for file in manifest.named_files() {
        if !inside(&file) {
          return Answered::Refused {
            why: format!("its manifest names {file}, which is not inside its folder"),
          };
        }

        let to = staged.join(&file);

        if let Some(parent) = to.parent()
          && let Err(why) = std::fs::create_dir_all(parent)
        {
          return Answered::Refused {
            why: format!("{}: {why}", parent.display()),
          };
        }

        if let Err(why) = std::fs::copy(source.join(&file), &to) {
          return Answered::Refused {
            why: format!("{file} could not be copied: {why}"),
          };
        }
      }
    }
    None => {
      if let Err(why) = copy_tree(&source, &staged) {
        return Answered::Refused {
          why: format!("{} could not be copied: {why}", source.display()),
        };
      }
    }
  }

  let files = match description(&staged) {
    Ok(files) => files,
    Err(why) => {
      return Answered::Refused {
        why: format!("the app {tool}'s placed version: {why}"),
      };
    }
  };
  let placed = owned.versions(tool).join(&version);

  if let Err(why) = std::fs::rename(&staged, &placed) {
    return Answered::Refused {
      why: format!("{} could not be placed: {why}", placed.display()),
    };
  }

  staging.0 = None;
  Answered::Placed { version, files }
}

// The tool's pointer moved onto a version standing, written whole; then every version but it and the newest kept
// older than it taken away, a version still being staged left as it is.
fn point(owned: &Owned, tool: &str, version: &str) -> Answered {
  let dir = owned.versions(tool).join(version);

  if !dir.join(MANIFEST).is_file() {
    return Answered::Refused {
      why: format!("version {version} of the app {tool} is not placed on this machine"),
    };
  }

  let pointers = owned.root.join("registry");
  let pointer = json!({
    "name": tool,
    "version": version,
    "dir": dir,
    "owner": owned.owner,
    "putAt": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |at| at.as_millis() as u64),
  });
  let staged = pointers.join(format!(".{tool}.json.next"));
  let written = std::fs::create_dir_all(&pointers)
    .and_then(|()| std::fs::write(&staged, pointer.to_string()))
    .and_then(|()| std::fs::rename(&staged, owned.pointer(tool)));

  if let Err(why) = written {
    return Answered::Refused {
      why: format!("the app {tool}'s pointer could not be written: {why}"),
    };
  }

  let Kept { kept, .. } = owned.kept(tool);
  let keeping: Vec<String> = kept.into_iter().take(KEEP).collect();

  for standing in owned.standing(tool) {
    if standing != version && !keeping.contains(&standing) {
      let _ = std::fs::remove_dir_all(owned.versions(tool).join(standing));
    }
  }

  Answered::Pointed {
    version: version.to_owned(),
    kept: keeping,
  }
}

// Every tool the owner keeps versions of, each with its version pointed at and those kept beside it.
fn versions(owned: &Owned) -> Answered {
  let tools = std::fs::read_dir(owned.root.join("tools"))
    .into_iter()
    .flatten()
    .flatten()
    .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
    .filter_map(|entry| entry.file_name().into_string().ok())
    .filter(|name| plain_name(name))
    .map(|tool| {
      let kept = owned.kept(&tool);

      (tool, kept)
    })
    .collect();

  Answered::Versions { tools }
}

// The tool's pointer and every version taken away; its source and its data stand.
fn take(owned: &Owned, tool: &str) -> Answered {
  let pointer = owned.pointer(tool);
  let versions = owned.versions(tool);

  if !pointer.exists() && !versions.exists() {
    return Answered::Refused {
      why: format!("this machine keeps no app {tool}"),
    };
  }

  for removed in [std::fs::remove_file(&pointer), std::fs::remove_dir_all(&versions)] {
    if let Err(why) = removed
      && why.kind() != std::io::ErrorKind::NotFound
    {
      return Answered::Refused {
        why: format!("the app {tool} could not be taken away: {why}"),
      };
    }
  }

  Answered::Taken {}
}

/// One keeping asked on the passage's first line and answered on `output`.
pub async fn keep(
  runner: Runner,
  first: &str,
  mut input: impl AsyncRead + Unpin,
  mut output: impl AsyncWrite + Unpin,
) {
  let answered = match serde_json::from_str::<Line>(first) {
    Err(why) => Answered::Refused {
      why: format!("the asking does not read: {why}"),
    },
    Ok(line) if runner.owner.as_deref().is_some_and(|owner| owner != line.owner) => Answered::Refused {
      why: format!("this runner keeps no app of the account {}", line.owner),
    },
    Ok(line) if !plain_name(&line.owner) => Answered::Refused {
      why: format!("{:?} is no owner's name", line.owner),
    },
    Ok(line) => {
      let owned = Owned::of(&runner, &line.owner);
      let named = match &line.asked {
        Asked::Place { tool } | Asked::Take { tool } => vec![("tool", tool)],
        Asked::Describe { tool, version } | Asked::Point { tool, version } => {
          vec![("tool", tool), ("version", version)]
        }
        Asked::Versions {} => Vec::new(),
      };

      match named.iter().find(|(_, name)| !plain_name(name)) {
        Some((what, name)) => Answered::Refused {
          why: format!("{name:?} is no {what}'s name"),
        },
        None => match &line.asked {
          Asked::Place { tool } => place(&runner, &owned, tool, &mut input, &mut output).await,
          Asked::Describe { tool, version } => match description(&owned.versions(tool).join(version)) {
            Ok(files) => Answered::Described {
              version: version.clone(),
              files,
            },
            Err(why) => Answered::Refused {
              why: format!("version {version} of the app {tool}: {why}"),
            },
          },
          Asked::Point { tool, version } => point(&owned, tool, version),
          Asked::Versions {} => versions(&owned),
          Asked::Take { tool } => take(&owned, tool),
        },
      }
    }
  };

  let _ = output.write_all(answered.line().as_bytes()).await;
  let _ = output.flush().await;
}
