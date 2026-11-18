//! What a job's command or file tool writes into a place the job guard keeps (docs/live-claude-guard.md,
//! 「The same hook keeps a job to its own checkout」, 「The same hook keeps STRUCTURE.md the owner's」): the
//! owner's statement document, and another worker's checkout. It is a reading of text: a redirect, a program
//! that writes files, an in-place editor, git changing a tree, and a script handed to an interpreter on the
//! command line or by a heredoc. A command that only reads — `cat`, `grep`, `git diff` — writes nothing.

use std::sync::LazyLock;

use regex::Regex;

use super::reading::{self, expand_assigned, program_of, read_command};
use super::records::{IN_PLACE_EDITORS, INNER_TEXT_DEPTH, WRITER_WORDS, in_place_flag, spellings};
use super::shell::SHELLS;

/// The document only the owner's own request changes, however a path to it is spelled.
pub const STRUCTURE_DOCUMENT: &str = "STRUCTURE.md";

// Programs that write only where their last word or their target-directory flag points, reading every other
// file they are named: a copy out of a place is no write into it.
const COPIERS: [&str; 4] = ["cp", "rsync", "install", "ln"];

// Programs that run a script handed to them inline.
const INTERPRETERS: [&str; 7] = ["python", "python3", "node", "bun", "perl", "ruby", "deno"];

/// A place a command may write into, as a reading of its text: what it is called in a refusal, what names
/// it, the git subcommands that count as changing it, and whether a `cd` into it makes every relative path
/// it.
pub trait WriteTarget {
  fn label(&self) -> String;
  fn names(&self, text: &str) -> bool;
  fn git_writers(&self) -> &'static [&'static str];
  fn follows_cd(&self) -> bool;
}

/// The statement document.
pub struct Structure;

impl WriteTarget for Structure {
  fn label(&self) -> String {
    STRUCTURE_DOCUMENT.into()
  }

  fn names(&self, text: &str) -> bool {
    text.contains(STRUCTURE_DOCUMENT)
  }

  fn git_writers(&self) -> &'static [&'static str] {
    &["rm", "mv", "checkout", "restore", "apply"]
  }

  fn follows_cd(&self) -> bool {
    false
  }
}

/// Another worker's checkout: its worker, the repository it is a checkout of, and its path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForeignCheckout {
  pub worker: String,
  pub repository: String,
  pub cwd: String,
}

/// The checkouts a job's writes are judged against: the paths it writes in — its own checkout and those it
/// borrowed — and every other worker's checkout on its machine. A path is the deepest checkout's that holds
/// it, since one checkout may stand inside another's directory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Checkouts {
  pub own: Vec<String>,
  pub foreign: Vec<ForeignCheckout>,
}

impl Checkouts {
  fn paths(&self) -> impl Iterator<Item = &str> {
    self
      .own
      .iter()
      .map(String::as_str)
      .chain(self.foreign.iter().map(|checkout| checkout.cwd.as_str()))
  }
}

// A path's boundary: what may follow a directory's name where the text names that directory itself.
const BOUNDARY: &str = r#"(?:[/\s"';&|)]|$)"#;

fn trimmed(path: &str) -> &str {
  path.trim_end_matches('/')
}

// Whether a path lies inside a directory or is the directory itself.
fn within(path: &str, root: &str) -> bool {
  let root = trimmed(root);

  path == root || path.starts_with(&format!("{root}/"))
}

// Another worker's checkout as a command may name it: its path as it stands and under the home, each matched
// at a boundary, so a support worktree whose path begins with its base's is never read as the base; and a
// path naming a checkout that stands inside this one is that checkout's, not this one's.
struct Foreign<'a> {
  checkout: &'a ForeignCheckout,
  patterns: Vec<Regex>,
  nested: Vec<Regex>,
}

impl<'a> Foreign<'a> {
  fn of(checkout: &'a ForeignCheckout, checkouts: &Checkouts, home: &str) -> Self {
    let at_boundary = |path: &str, anchor: &str| {
      spellings(path, home)
        .iter()
        .filter_map(|spelling| Regex::new(&format!("{anchor}{}{BOUNDARY}", regex::escape(spelling))).ok())
        .collect::<Vec<_>>()
    };
    let nested = checkouts
      .paths()
      .filter(|path| trimmed(path) != trimmed(&checkout.cwd) && within(trimmed(path), &checkout.cwd))
      .flat_map(|path| at_boundary(path, "^"))
      .collect();

    Self {
      checkout,
      patterns: at_boundary(&checkout.cwd, ""),
      nested,
    }
  }
}

impl WriteTarget for Foreign<'_> {
  fn label(&self) -> String {
    format!("{} ({}'s checkout)", self.checkout.cwd, self.checkout.worker)
  }

  fn names(&self, text: &str) -> bool {
    self.patterns.iter().any(|pattern| {
      pattern
        .find_iter(text)
        .any(|found| !self.nested.iter().any(|inner| inner.is_match(&text[found.start()..])))
    })
  }

  fn git_writers(&self) -> &'static [&'static str] {
    &[
      "add",
      "commit",
      "rm",
      "mv",
      "checkout",
      "switch",
      "restore",
      "reset",
      "apply",
      "stash",
      "merge",
      "rebase",
      "revert",
      "cherry-pick",
      "pull",
    ]
  }

  fn follows_cd(&self) -> bool {
    true
  }
}

// The letters and the words an interpreter carries a script under: python's `-c`, and the evaluate and
// print flags of node, bun, deno, perl and ruby.
const SCRIPT_LETTERS: [char; 3] = ['c', 'e', 'p'];
const SCRIPT_WORDS: [&str; 2] = ["--eval", "--print"];

// Every script an interpreter's command line hands it inline. Perl and Ruby take their flags combined —
// `-ne`, `-pe`, `-nlae` — so a cluster is read letter by letter: a letter that carries a script takes the
// rest of its cluster as that script, or the next word where the cluster ends at it, and the reading goes on
// through the cluster either way. In an in-place editor's cluster an `i` takes everything behind it as the
// backup suffix, so the reading of that cluster ends there.
fn inline_scripts(program: &str, args: &[String]) -> Vec<String> {
  let mut scripts = vec![];
  let suffix_after_i = IN_PLACE_EDITORS.contains(&program);

  for (at, arg) in args.iter().enumerate() {
    if let Some(long) = arg.strip_prefix("--") {
      let (name, given) = match long.split_once('=') {
        Some((name, given)) => (name, Some(given)),
        None => (long, None),
      };

      if SCRIPT_WORDS.contains(&format!("--{name}").as_str()) {
        scripts.push(given.map_or_else(|| args.get(at + 1).cloned().unwrap_or_default(), str::to_owned));
      }

      continue;
    }

    let Some(cluster) = arg.strip_prefix('-') else {
      continue;
    };

    for (letter, ch) in cluster.char_indices() {
      if ch == 'i' && suffix_after_i {
        break;
      }

      if SCRIPT_LETTERS.contains(&ch) {
        let rest = &cluster[letter + ch.len_utf8()..];

        scripts.push(if rest.is_empty() {
          args.get(at + 1).cloned().unwrap_or_default()
        } else {
          rest.to_owned()
        });
      }
    }
  }

  scripts
}

// Whether an interpreter reads its script from where a heredoc or a pipe puts it. A script file standing
// among the arguments is a file of its own, out of this reading's sight; `-` standing there is the
// interpreter's own name for its standard input.
fn reads_script_from_stdin(args: &[String]) -> bool {
  args
    .iter()
    .find(|arg| *arg == "-" || !arg.starts_with('-'))
    .is_none_or(|first| first == "-")
}

// A script's write calls are found by these patterns rather than by parsing the script: the script may be
// Python, JavaScript, Perl or Ruby, no maintained library reads all of them, and the reading asks of it only
// which files its write calls name.
macro_rules! pattern {
  ($name:ident, $text:expr) => {
    static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($text).expect("a write-call pattern"));
  };
}

pattern!(PY_OPEN, r#"\bopen\s*\(\s*([^,()]+?)\s*,\s*(?:mode\s*=\s*)?['"]([rwabxt+]+)['"]"#);
pattern!(PY_WRITING_MODE, r"[wax+]");
pattern!(
  PERL_OPEN_TWO,
  r#"\bopen\s*\(\s*(?:my\s+)?[\w$]+\s*,\s*['"]\s*(?:>>?|\+[<>])\s*([^'"\s][^'"]*)['"]\s*\)"#
);
pattern!(
  PERL_OPEN_THREE,
  r#"\bopen\s*\(\s*(?:my\s+)?[\w$]+\s*,\s*['"](?:>>?|\+[<>])['"]\s*,\s*([^,()]+)"#
);
pattern!(
  WRITE_TEXT,
  r#"([A-Za-z_][\w.]*(?:\(\s*['"][^'"]+['"]\s*\))?)\s*\.\s*write_(?:text|bytes)\s*\("#
);
pattern!(
  WRITE_FILE,
  r"(?:Bun\.write|(?:fs\.)?writeFileSync|(?:fs\.)?writeFile|File\.write|IO\.write|\.writeFile)\s*\(\s*([^,()]+)"
);
pattern!(LITERAL, r#"^(?:[A-Za-z_][\w.]*)?\(?\s*['"]([^'"]+)['"]\s*\)?$"#);
pattern!(NAME, r"^[A-Za-z_]\w*$");
pattern!(QUOTED, r#"['"]([^'"]+)['"]"#);

// The paths an inline script writes, as the script's own text spells them, and whether a write call was made
// whose file could not be read.
struct WriteCalls {
  paths: Vec<String>,
  unresolved: bool,
}

fn write_calls(script: &str) -> WriteCalls {
  let mut calls = WriteCalls {
    paths: vec![],
    unresolved: false,
  };
  let add = |calls: &mut WriteCalls, expr: &str| {
    let named = named_paths(expr, script);

    calls.unresolved |= named.is_empty();
    calls.paths.extend(named);
  };

  // Python's `open(<file>, '<mode>')`, a write where the mode writes, appends or updates.
  for call in PY_OPEN.captures_iter(script) {
    if PY_WRITING_MODE.is_match(&call[2]) {
      add(&mut calls, &call[1]);
    }
  }

  // Perl's `open(F, ">file")`, and its three-argument `open(F, ">", <file>)`.
  for call in PERL_OPEN_TWO.captures_iter(script) {
    calls.paths.push(call[1].to_owned());
  }

  for call in PERL_OPEN_THREE.captures_iter(script) {
    add(&mut calls, &call[1]);
  }

  for call in WRITE_TEXT.captures_iter(script) {
    add(&mut calls, &call[1]);
  }

  for call in WRITE_FILE.captures_iter(script) {
    add(&mut calls, &call[1]);
  }

  calls
}

// A path a write call named: a literal as it stands, or every literal the script gave that name — by an
// assignment, or by the list a loop walks.
fn named_paths(expr: &str, script: &str) -> Vec<String> {
  let trimmed = expr.trim();

  if let Some(literal) = LITERAL.captures(trimmed) {
    return vec![literal[1].to_owned()];
  }

  if !NAME.is_match(trimmed) {
    return vec![];
  }

  let mut found = vec![];
  let given = Regex::new(&format!(r#"\b{trimmed}\s*=\s*(?:[A-Za-z_][\w.]*)?\(?\s*['"]([^'"]+)['"]"#));
  let walked = Regex::new(&format!(r"for\s+{trimmed}\s+in\s*[\[(]([^\])]*)[\])]"));

  if let Ok(given) = given {
    found.extend(given.captures_iter(script).map(|given| given[1].to_owned()));
  }

  if let Ok(walked) = walked {
    for list in walked.captures_iter(script) {
      found.extend(QUOTED.captures_iter(&list[1]).map(|item| item[1].to_owned()));
    }
  }

  found
}

// Whether a script writes into the target: one of its write calls names it, or it names the target and makes
// a write call whose file the reading cannot resolve — refused rather than guessed at. A script that opens
// the target only to read it makes no write call, and is not a write.
fn script_writes_target(script: &str, target: &dyn WriteTarget) -> bool {
  let calls = write_calls(script);

  calls.paths.iter().any(|path| target.names(path)) || (calls.unresolved && target.names(script))
}

// The words a writing program writes at: a copier's destination — the value of its target-directory flag, else
// its last plain word — and every word of any other.
fn written_words<'a>(program: &str, args: &'a [String]) -> Vec<&'a String> {
  if !COPIERS.contains(&program) {
    return args.iter().collect();
  }

  if let Some(at) = args.iter().position(|arg| arg == "-t" || arg == "--target-directory") {
    return args.get(at + 1).into_iter().collect();
  }

  if let Some(target) = args.iter().find(|arg| arg.starts_with("--target-directory=")) {
    return vec![target];
  }

  args.iter().rev().find(|arg| !arg.starts_with('-')).into_iter().collect()
}

// A path spelled relative to wherever the shell stands.
fn relative_path(word: &str) -> bool {
  !word.is_empty() && !word.starts_with(['-', '/', '~', '$', '&'])
}

// The text a shell is handed with `-c`, or the text given to `eval`.
fn handed_text(run: &reading::Run) -> Option<String> {
  if run.program == "eval" {
    Some(run.args.join(" "))
  } else if SHELLS.contains(&run.program.as_str()) {
    reading::shell_text(run).cloned()
  } else {
    None
  }
}

/// Why this command would write into the target, or nothing.
pub fn write_reason(command: &str, target: &dyn WriteTarget) -> Option<String> {
  reason_at(command, target, 0)
}

fn reason_at(original: &str, target: &dyn WriteTarget, depth: usize) -> Option<String> {
  let command = expand_assigned(original);

  if !target.names(&command) {
    return None;
  }

  let read = read_command(&command);
  let label = target.label();
  // Whether the shell stands inside the target, after a `cd` into it.
  let mut inside = false;

  for segment in &read.commands {
    let named = |word: &String| target.names(word) || (inside && relative_path(word));

    if segment.redirects.iter().any(named) {
      return Some(format!("it writes into {label}"));
    }

    let Some(run) = program_of(&segment.words) else {
      continue;
    };
    let program = run.program.as_str();

    if target.follows_cd() && (program == "cd" || program == "pushd") {
      inside = run.args.iter().any(|arg| target.names(arg))
        || (inside && run.args.iter().any(|arg| relative_path(arg)));
      continue;
    }

    let arg_named = run.args.iter().any(named);

    if WRITER_WORDS.contains(&program) && written_words(program, &run.args).into_iter().any(named) {
      return Some(format!("it copies, moves, creates or removes files in {label}"));
    }

    if arg_named && IN_PLACE_EDITORS.contains(&program) && run.args.iter().any(|arg| in_place_flag(arg)) {
      return Some(format!("it edits {label} in place"));
    }

    if (arg_named || inside)
      && program == "git"
      && run.args.iter().any(|arg| target.git_writers().contains(&arg.as_str()))
    {
      return Some(format!("it changes {label} through git"));
    }

    if INTERPRETERS.contains(&program) {
      let scripts = inline_scripts(program, &run.args);
      let writes = if scripts.is_empty() {
        reads_script_from_stdin(&run.args)
          && read.heredocs.iter().any(|held| script_writes_target(&held.text, target))
      } else {
        scripts.iter().any(|script| script_writes_target(script, target))
      };

      if writes {
        return Some(format!("it runs a script that names {label} and writes"));
      }
    }

    if depth < INNER_TEXT_DEPTH
      && let Some(inner) = handed_text(&run)
      && let Some(refused) = reason_at(&inner, target, depth + 1)
    {
      return Some(refused);
    }
  }

  None
}

/// Why this command would change the statement document, or nothing.
pub fn structure_command_write(command: &str) -> Option<String> {
  write_reason(command, &Structure)
}

/// Why a file tool's call on this path would change the statement document, or nothing.
pub fn structure_file_write(path: &str) -> Option<String> {
  (path == STRUCTURE_DOCUMENT || path.ends_with(&format!("/{STRUCTURE_DOCUMENT}")))
    .then(|| format!("it edits {STRUCTURE_DOCUMENT}"))
}

/// What a refusal of a change to the statement document says.
pub fn structure_refusal(why: &str) -> String {
  format!(
    "refused: {why}. {STRUCTURE_DOCUMENT} is the owner's statement of the system, and only a job whose item the owner asked for himself and whose title or brief names it may change it. Where the code differs from it, report the difference instead (docs/live-claude-guard.md, 「The same hook keeps STRUCTURE.md the owner's」)."
  )
}

/// The checkout a command would write into that is another worker's, with why, or nothing; `home` is the home
/// the command's paths are spelled under.
pub fn foreign_command_write<'a>(
  command: &str,
  checkouts: &'a Checkouts,
  home: &str,
) -> Option<(&'a ForeignCheckout, String)> {
  checkouts.foreign.iter().find_map(|checkout| {
    write_reason(command, &Foreign::of(checkout, checkouts, home)).map(|why| (checkout, why))
  })
}

/// The checkout a file tool's target lies in that is another worker's, or nothing: the deepest checkout
/// holding the path, where that one is another worker's.
pub fn foreign_file_write<'a>(path: &str, checkouts: &'a Checkouts) -> Option<&'a ForeignCheckout> {
  let deepest = checkouts
    .paths()
    .filter(|root| within(path, root))
    .max_by_key(|root| trimmed(root).len())?;

  checkouts.foreign.iter().find(|checkout| checkout.cwd == deepest)
}

/// What a refusal of a write into another worker's checkout says.
pub fn foreign_refusal(checkout: &ForeignCheckout, why: &str) -> String {
  format!(
    "refused: {why} — {cwd} is {worker}'s checkout, and a job writes only in its own checkout and in one it has borrowed, since what is open in a checkout is parked as its holder's own when that holder stops. To change that repository, borrow one of its checkouts for this job with bmo job borrow {repository}, which hands you a free one (docs/worker-slots.md, 「A job writes only in its own checkout」).",
    cwd = checkout.cwd,
    worker = checkout.worker,
    repository = checkout.repository,
  )
}
