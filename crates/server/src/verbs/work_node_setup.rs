//! The workplace on a work node, stood up and read again (docs/work-nodes.md, 「The workplace on a node」):
//! each step checked before the next, and everything still missing said one thing per line, so a node
//! with anything missing takes no work. The steps are this module's; the node, the store and the
//! machine's own roots are handed in, so a test runs them against a shell of its own.

use futures_util::future::BoxFuture;

use crate::watches::links::node_path;

/// A word as a POSIX shell reads it back as itself.
pub fn shell_word(word: &str) -> String {
  let plain =
    !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));

  if plain { word.to_owned() } else { format!("'{}'", word.replace('\'', r#"'"'"'"#)) }
}

/// What a command run on the node came to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShellRun {
  pub code: i32,
  pub stdout: String,
  pub stderr: String,
}

/// The node's shell the setup runs its commands in.
pub trait SetupShell: Sync {
  fn run<'a>(&'a self, command: &'a str) -> BoxFuture<'a, ShellRun>;
}

/// One account's login as this machine keeps it — its own configuration directory, or the machine's for
/// a token account — whether this machine holds its token for the work nodes, and the command that
/// makes one.
#[derive(Clone, Debug)]
pub struct SetupLogin {
  pub account: String,
  /// Whether the path is the account's own configuration directory rather than the machine's.
  pub own_directory: bool,
  pub path: String,
  pub node_token: bool,
  pub token_command: String,
}

/// A checkout placed on the node, with the remote its base clones from.
#[derive(Clone, Debug)]
pub struct SetupCheckout {
  pub worker: String,
  pub cwd: String,
  pub remote: Option<String>,
}

/// What the setup is handed about where it works.
pub struct SetupPlace<'a> {
  pub node: String,
  /// The workplace's root as written, which may be under the node's home.
  pub root: String,
  /// This machine's deploy tree, whose commit is copied to the node.
  pub deploy: String,
  pub checkouts: Vec<SetupCheckout>,
  pub logins: Vec<SetupLogin>,
  /// A path of this machine as the node reads it, once its home and the workplace's root are known.
  pub to_node: &'a (dyn Fn(&str, &str, &str) -> String + Sync),
  /// The destination of the copy of the tree.
  pub destination: String,
  /// This machine's herdr, which a node without one is given.
  pub herdr: Option<String>,
  /// This machine's Rust version, which the node's build takes.
  pub rust: Option<String>,
  /// This machine's bun version, which the node's must be: bun reads the repository's own configuration
  /// and lockfile, which another bun may not.
  pub bun: Option<String>,
  /// The herdr session the node's agents stand in, apart from any session a person keeps there.
  pub session: String,
}

/// What each step may do beyond reading: install a missing tool, copy the code, build, clone.
pub trait SetupHands: Sync {
  fn install(&self) -> bool;
  /// Whether the code is copied and built, or only found there: a check reads, a setup and a sync write.
  fn copy_code(&self) -> bool;
  fn copy<'a>(&'a self, from: &'a str, to: &'a str) -> BoxFuture<'a, (bool, String)>;
  fn add_machine(&self) -> BoxFuture<'_, (bool, String)>;
  fn say(&self, line: &str);
}

/// What a setup came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupOutcome {
  pub home: Option<String>,
  pub root: Option<String>,
  pub missing: Vec<String>,
  pub herdr_socket: Option<String>,
}

/// The crates a node builds its own binaries from, each by its folder in the repository, which it keeps in
/// the workspace `crates/node-link/node-build.toml` makes (docs/work-nodes.md, 「The workplace on a node」).
pub const NODE_CRATES: [&str; 4] =
  ["crates/agent-host", "crates/store", "crates/pane-head", "crates/node-link"];

/// What the deploy tree's copy on a node leaves out: what is built or installed there, and what no job on
/// a node reads.
pub const COPY_LEFT_OUT: [&str; 10] = [
  "/.git",
  "node_modules/",
  "/target/",
  "/bin/",
  "/android/",
  "/client/.output/",
  "/client/.nuxt/",
  "/data/",
  "/.env*",
  "*.log",
];

// The binaries a node builds for itself.
const NODE_BINARIES: [&str; 3] = ["pane-head", "agent-host", "node-link"];

// The search path every command of the setup runs with, the tools installed into the account first.
fn with_path(command: &str) -> String {
  format!(r#"export PATH="$HOME/.bun/bin:$HOME/.local/bin:$HOME/.cargo/bin:$PATH"; {command}"#)
}

// An installer fetched into a file and run from it with nothing on its standing input: run as `curl … |
// bash`, a program it starts inherits the pipe the script came through and may wait on it.
fn installer(url: &str, args: &str) -> String {
  let name: String = url.chars().map(|one| if one.is_ascii_alphanumeric() { one } else { '-' }).collect();
  let script = format!("/tmp/bmo-install-{name}.sh");

  format!(
    "curl -fsSL {} -o {} && bash {} {args} </dev/null; status=$?; rm -f {}; exit $status",
    shell_word(url),
    shell_word(&script),
    shell_word(&script),
    shell_word(&script)
  )
}

// Claude Code's own installer, bounded, and its binary put where the installer puts it where its own
// install step did not finish: under a login with no terminal that step can spin without end with the
// binary it fetched already whole beside it.
fn claude_installer() -> String {
  let place = [
    r#"f=$(ls -t "$HOME"/.claude/downloads/claude-*-linux-* 2>/dev/null | head -1)"#,
    r#"if [ -n "$f" ]; then v=$(basename "$f" | sed "s/^claude-\([0-9.]*\)-.*/\1/"); mkdir -p "$HOME/.local/share/claude/versions" "$HOME/.local/bin" && cp "$f" "$HOME/.local/share/claude/versions/$v" && ln -sfn "$HOME/.local/share/claude/versions/$v" "$HOME/.local/bin/claude"; fi"#,
  ]
  .join("; ");

  format!(
    r#"curl -fsSL https://claude.ai/install.sh -o /tmp/bmo-install-claude.sh && timeout 300 bash /tmp/bmo-install-claude.sh </dev/null; rm -f /tmp/bmo-install-claude.sh; test -x "$HOME/.local/bin/claude" || {{ {place}; }}"#
  )
}

// One tool a workplace needs: how it is installed into the account where it is missing — none where it
// is the machine's to have — and how its version is read where it must be this machine's.
struct Tool {
  name: &'static str,
  install: Option<String>,
  version: Option<(&'static str, String)>,
}

fn tools(bun: Option<&str>) -> Vec<Tool> {
  vec![
    Tool {
      name: "git",
      install: None,
      version: None,
    },
    Tool {
      name: "rsync",
      install: None,
      version: None,
    },
    Tool {
      name: "curl",
      install: None,
      version: None,
    },
    Tool {
      name: "bun",
      install: Some(installer(
        "https://bun.sh/install",
        &bun.map(|bun| format!("bun-v{bun}")).unwrap_or_default(),
      )),
      version: bun.map(|bun| ("bun --version", bun.to_owned())),
    },
    Tool {
      name: "cargo",
      install: Some(installer("https://sh.rustup.rs", "-y --profile minimal")),
      version: None,
    },
    Tool {
      name: "claude",
      install: Some(claude_installer()),
      version: None,
    },
  ]
}

/// The herdr session's socket as the node's herdr lists it, or none where it lists none.
pub fn session_socket(listing: &str, session: &str) -> Option<String> {
  listing.lines().find_map(|line| {
    let words: Vec<&str> = line.split_whitespace().collect();

    (words.first() == Some(&session) && words.len() >= 4).then(|| words[words.len() - 1].to_owned())
  })
}

fn last_lines(text: &str, count: usize) -> String {
  let lines: Vec<&str> = text.trim().split('\n').collect();

  lines[lines.len().saturating_sub(count)..].join(" ")
}

fn join(base: &str, part: &str) -> String {
  std::path::Path::new(base).join(part).to_string_lossy().into_owned()
}

// A command whose connection failed is a step that failed, said with the rest, never the end of the
// setup.
async fn found(shell: &dyn SetupShell, tool: &str) -> bool {
  shell.run(&with_path(&format!("command -v {} >/dev/null", shell_word(tool)))).await.code == 0
}

/// Every step of the setup in order. A step that cannot be met is said and the rest still run where they
/// can, so one pass names everything missing; a node whose login fails names that alone.
pub async fn set_up_workplace(
  shell: &dyn SetupShell,
  place: &SetupPlace<'_>,
  hands: &dyn SetupHands,
) -> SetupOutcome {
  let mut missing = Vec::new();
  let home_run = shell.run(r#"printf '%s\n' "$HOME""#).await;
  let home = home_run.stdout.trim().to_owned();

  if home_run.code != 0 || !home.starts_with('/') {
    let said = home_run.stderr.trim().split('\n').next_back().filter(|line| !line.is_empty());

    return SetupOutcome {
      home: None,
      root: None,
      missing: vec![format!("login: {}", said.unwrap_or("the node did not answer"))],
      herdr_socket: None,
    };
  }

  let root = node_path(&place.root, Some(&home));
  let deploy = join(&root, "deploy");
  let data = join(&root, "data");
  let node = &place.node;

  hands.say(&format!("{node}: home {home}, workplace {root}"));

  // The tools, each installed into the account where it is missing and the setup may install.
  for tool in tools(place.bun.as_deref()) {
    let standing = found(shell, tool.name).await;
    let version = match (&tool.version, standing) {
      (Some((ask, _)), true) => Some(shell.run(&with_path(ask)).await.stdout.trim().to_owned()),
      _ => None,
    };
    let wanted = tool.version.as_ref().map(|(_, want)| want.clone());

    if standing && (wanted.is_none() || version == wanted) {
      continue;
    }

    if standing && !hands.install() {
      missing.push(format!(
        "{}: {} on the node, where this machine runs {}; run the setup with --install",
        tool.name,
        version.unwrap_or_default(),
        wanted.unwrap_or_default()
      ));
      continue;
    }

    if hands.install()
      && let Some(install) = &tool.install
    {
      hands.say(&format!("{node}: installing {}", tool.name));

      let installed = shell.run(&with_path(install)).await;
      let now = match &tool.version {
        Some((ask, _)) => Some(shell.run(&with_path(ask)).await.stdout.trim().to_owned()),
        None => None,
      };

      if installed.code == 0 && found(shell, tool.name).await && (wanted.is_none() || now == wanted) {
        continue;
      }
    }

    missing.push(match &tool.install {
      None => format!("{}: not on the node, and the machine's to install", tool.name),
      Some(_) if hands.install() => format!("{}: not installed (its installer failed)", tool.name),
      Some(_) => format!("{}: not installed; run the setup with --install", tool.name),
    });
  }

  // Bun, and every Claude Code release built on it, needs a processor with AVX: on one without, as a
  // virtual machine on a generic processor type is, it crashes on its first line.
  if shell.run("grep -q -w avx /proc/cpuinfo").await.code != 0 {
    missing.push("cpu: the node's processor offers no AVX, which bun and Claude Code need; a virtual machine takes it with a processor type that passes the host's through".into());
  } else if found(shell, "bun").await
    && !shell.run(&with_path("bun -e 'console.log(6 * 7)'")).await.stdout.contains("42")
  {
    missing.push("bun: installed on the node, but it does not run there".into());
  }

  // The Rust toolchain, brought to this machine's version through rustup.
  if let Some(rust) = &place.rust {
    // The toolchain is named for the build alone, so the account's own default stays as it was.
    let rustc = shell
      .run(&with_path(&format!("RUSTUP_TOOLCHAIN={} rustc --version", shell_word(rust))))
      .await
      .stdout
      .trim()
      .to_owned();

    if !rustc.starts_with(&format!("rustc {rust} ")) {
      let pinned = if hands.install() {
        let bring = format!(
          "if command -v rustup >/dev/null; then rustup toolchain install {} --profile minimal; else {}; fi",
          shell_word(rust),
          installer(
            "https://sh.rustup.rs",
            &format!("-y --profile minimal --default-toolchain {}", shell_word(rust))
          )
        );

        Some(shell.run(&with_path(&bring)).await)
      } else {
        None
      };

      if pinned.is_none_or(|pinned| pinned.code != 0) {
        missing.push(format!(
          "rust: {} on the node, where this machine builds with {rust}{}",
          if rustc.is_empty() { "none" } else { &rustc },
          if hands.install() {
            " (rustup could not bring it)"
          } else {
            "; run the setup with --install"
          }
        ));
      }
    }
  }

  let folders = [
    deploy.clone(),
    join(&data, "run"),
    join(&data, "agent-places"),
    join(&data, "env"),
    join(&root, "benches"),
  ];

  shell
    .run(&format!(
      "mkdir -p {}",
      folders.iter().map(|folder| shell_word(folder)).collect::<Vec<_>>().join(" ")
    ))
    .await;

  // The code: the deploy tree's commit, installed there, and the node's own binaries built from it.
  if !hands.copy_code() {
    let standing = NODE_BINARIES
      .iter()
      .map(|binary| format!("test -x {}", shell_word(&join(&deploy, &format!("bin/{binary}")))))
      .collect::<Vec<_>>()
      .join(" && ");

    if shell.run(&standing).await.code != 0 {
      missing
        .push("code: the deploy tree's copy or its binaries are not on the node; run the setup".into());
    }
  } else {
    hands.say(&format!("{node}: copying the code"));

    let (copied, said) = hands
      .copy(&format!("{}/", place.deploy), &format!("{}:{deploy}/", place.destination))
      .await;

    if !copied {
      missing.push(format!("code: the deploy tree could not be copied: {said}"));
    } else {
      let install = shell
        .run(&with_path(&format!("cd {} && bun install --frozen-lockfile", shell_word(&deploy))))
        .await;

      if install.code != 0 {
        missing.push(format!("code: bun install failed: {}", last_lines(&install.stderr, 3)));
      }

      hands.say(&format!("{node}: building pane-head, agent-host and node-link there"));

      // In a workspace of their own beside the code, from the crates they are made of and the
      // repository's lockfile: the whole workspace names a git dependency a node may not reach.
      let build = join(&root, "build");
      let target = join(&root, "cargo-target");
      let toolchain = place
        .rust
        .as_ref()
        .map(|rust| format!("RUSTUP_TOOLCHAIN={} ", shell_word(rust)))
        .unwrap_or_default();
      // Each crate at the path it holds in the repository, so the paths between them read as they do there.
      let parent = |one: &str| one.rsplit_once('/').map_or("", |(above, _)| above).to_owned();
      let copies = NODE_CRATES
        .iter()
        .map(|one| {
          format!(
            "mkdir -p {above} && cp -r {} {above}/",
            shell_word(&join(&deploy, one)),
            above = shell_word(&join(&build, &parent(one)))
          )
        })
        .collect::<Vec<_>>()
        .join(" && ");
      let steps = [
        format!("rm -rf {}", shell_word(&build)),
        format!("mkdir -p {} {}", shell_word(&build), shell_word(&join(&deploy, "bin"))),
        copies,
        format!(
          "cp {} {}",
          shell_word(&join(&deploy, "crates/node-link/node-build.toml")),
          shell_word(&join(&build, "Cargo.toml"))
        ),
        format!(
          "cp {} {}",
          shell_word(&join(&deploy, "Cargo.lock")),
          shell_word(&join(&build, "Cargo.lock"))
        ),
        format!("cd {}", shell_word(&build)),
        format!("{toolchain}CARGO_TARGET_DIR={} cargo build --release -q", shell_word(&target)),
        format!(
          "cp {} {}/",
          NODE_BINARIES
            .iter()
            .map(|one| shell_word(&join(&target, &format!("release/{one}"))))
            .collect::<Vec<_>>()
            .join(" "),
          shell_word(&join(&deploy, "bin"))
        ),
        // The agent host standing on the old binary ends, and the link's far end starts the new one.
        format!(
          r#"(held=$(grep -o '"pid":[0-9]*' {} 2>/dev/null | cut -d: -f2); if [ -n "$held" ]; then kill "$held" 2>/dev/null; fi; true)"#,
          shell_word(&join(&data, "executions/agent-host/claim"))
        ),
      ];
      let built = shell.run(&with_path(&steps.join(" && "))).await;

      if built.code != 0 {
        missing.push(format!("binaries: the build failed: {}", last_lines(&built.stderr, 3)));
      }
    }
  }

  // herdr, and the session the agents stand in: this machine's herdr given to a node with none, and
  // herdr's own machine setup saving the node here, so herdr's sidebar shows it.
  if hands.copy_code()
    && let Some(herdr) = &place.herdr
    && !found(shell, "herdr").await
  {
    shell.run(r#"mkdir -p "$HOME/.local/bin""#).await;

    let (given, said) = hands
      .copy(herdr, &format!("{}:{}", place.destination, join(&home, ".local/bin/herdr")))
      .await;

    if !given {
      hands.say(&format!("{node}: herdr could not be copied: {said}"));
    }
  }

  if hands.copy_code() {
    let (added, said) = hands.add_machine().await;

    if !added && !said.to_lowercase().contains("already") {
      hands.say(&format!("{node}: herdr machine add said: {said}"));
    }
  }

  let mut herdr_socket = None;

  if found(shell, "herdr").await {
    let listing = shell.run(&with_path("herdr session list </dev/null")).await;

    herdr_socket = session_socket(&listing.stdout, &place.session);

    if herdr_socket.is_none() {
      shell
        .run(&with_path(&format!(
          "setsid -f herdr --session {} server </dev/null >{} 2>&1; sleep 2",
          shell_word(&place.session),
          shell_word(&join(&data, "herdr-server.log"))
        )))
        .await;
      herdr_socket = session_socket(
        &shell.run(&with_path("herdr session list </dev/null")).await.stdout,
        &place.session,
      );
    }

    if herdr_socket.is_none() {
      missing.push(format!("herdr: the session {} does not stand on the node", place.session));
    }
  } else {
    missing.push("herdr: not installed; add the node to herdr with herdr machine add".into());
  }

  // The checkouts placed there, cloned where they are not, set up, and trusted under every login.
  for checkout in &place.checkouts {
    let cwd = node_path(&checkout.cwd, Some(&home));
    let standing = shell.run(&format!("test -d {}", shell_word(&join(&cwd, ".git")))).await;

    if standing.code != 0 && !hands.copy_code() {
      missing.push(format!("{}: no checkout at {cwd}; run the setup", checkout.worker));
      continue;
    }

    if standing.code != 0 {
      let Some(remote) = &checkout.remote else {
        missing.push(format!(
          "{}: no checkout at {cwd}, and its base names no remote to clone",
          checkout.worker
        ));
        continue;
      };

      hands.say(&format!("{node}: cloning {} into {cwd}", checkout.worker));

      let cloned = shell
        .run(&with_path(&format!("git clone -q {} {}", shell_word(remote), shell_word(&cwd))))
        .await;

      if cloned.code != 0 {
        let said = cloned.stderr.trim().split('\n').next_back().unwrap_or_default().to_owned();

        missing.push(format!("{}: could not clone {remote}: {said}", checkout.worker));
        continue;
      }
    }

    if hands.copy_code() {
      let setup = shell
        .run(&with_path(&format!(
          r#"cd {} && if grep -q '"worktree:setup"' package.json 2>/dev/null; then bun run worktree:setup; fi"#,
          shell_word(&cwd)
        )))
        .await;

      if setup.code != 0 {
        missing.push(format!("{}: its setup failed: {}", checkout.worker, last_lines(&setup.stderr, 2)));
      }
    }

    for login in &place.logins {
      let file = if login.own_directory {
        join(&(place.to_node)(&login.path, &home, &root), ".claude.json")
      } else {
        join(&home, ".claude.json")
      };

      shell
        .run(&with_path(&format!(
          "{} trust {} {}",
          shell_word(&join(&deploy, "bin/agent-host")),
          shell_word(&file),
          shell_word(&cwd)
        )))
        .await;
    }

    let push = shell
      .run(&with_path(&format!(
        "cd {} && git push --dry-run -q origin HEAD:refs/heads/bmo-node-check",
        shell_word(&cwd)
      )))
      .await;

    if push.code != 0 {
      let said = push.stderr.trim().split('\n').next_back().unwrap_or_default().to_owned();

      missing.push(format!("{}: the node cannot push to its remote: {said}", checkout.worker));
    }
  }

  // Each login a run on the node goes out on, never a credential file copied: the account's token for
  // the work nodes, which this machine hands every run, or else a login made on the node in the
  // account's directory there.
  for login in &place.logins {
    if login.node_token {
      continue;
    }

    let path = (place.to_node)(&login.path, &home, &root);

    if login.own_directory
      && shell
        .run(&format!("test -s {}", shell_word(&join(&path, ".credentials.json"))))
        .await
        .code
        == 0
    {
      continue;
    }

    missing.push(format!(
      "login {}: no token for the work nodes; on this machine, run {}",
      login.account, login.token_command
    ));
  }

  SetupOutcome {
    home: Some(home),
    root: Some(root),
    missing,
    herdr_socket,
  }
}
