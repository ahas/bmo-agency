//! The research runners' lines as a run of each is composed (docs/launch-lines.md, 「Search runner」,
//! 「Browse runner」), where agency sends one out — the keepalive's ping on the line: the runner's standing
//! instructions in place of the CLI's own, written where the line hands them over; its standing MCP
//! configuration, written where nothing has written it; the tier its key is assigned on Claude Code and its
//! agent's own thinking and effort laid on it; and the directory it is deployed to. Each runner answers for
//! what is its own: its instructions, the tools it is allowed, the shape its answer comes back in, and its
//! configuration.

use std::path::PathBuf;

use harness_providers::claude_code::ClaudeCode;
use policy::browser::BROWSER;
use policy::research::RESEARCH;

use super::ordered::Ordered;
use super::token::{TOKEN_VARIABLE, listing};
use super::worker::{SELF_PRESERVATION, effort_of, reasoning_of, rulebook, system_prompt_file};
use super::{Trees, fault, publish_text, read_required, render};
use crate::Refusal;
use crate::control::tiers;
use crate::places::Ground;

/// The provider a runner's line is warmed on.
pub const PROVIDER: &str = "claude-code";

// The web reaches every runner through the deployment's own server and its toolbook, and no built-in tool
// is shown to any.
const WEB_TOOLS: &str = "mcp__web,mcp__toolbook";

// The rulebook every text a person reads is written to.
const PLAIN_WRITING: &str = "plain-writing";

/// One research runner: the line, the work key and the agent it is all at once.
pub trait Runner {
  fn name(&self) -> &'static str;

  /// What it is allowed of the servers it is handed.
  fn tools_allowed(&self) -> &'static str;

  /// Its standing instructions, or none where what they are made of cannot be had now.
  fn instructions(
    &self,
    ground: &Ground,
    trees: &Trees,
  ) -> impl Future<Output = Result<Option<String>, Refusal>>;

  /// Its standing MCP configuration, written where nothing has written it.
  fn mcp_config(&self, ground: &Ground, trees: &Trees) -> Result<PathBuf, Refusal>;
}

/// The search runner: the web and its pooled branches, and nothing else.
pub struct Search;

/// The browse runner: the browser's tools and the web.
pub struct Browse;

/// A runner's run as composed, less its prompt, which is put into its program (docs/agents.md, 「Runs
/// apart」).
pub struct RunnerRun {
  pub line: &'static str,
  pub model: String,
  pub effort: String,
  pub tier: String,
  pub system_prompt: PathBuf,
  pub mcp_config: PathBuf,
  pub tools_allowed: &'static str,
  pub thinking: bool,
  pub cwd: PathBuf,
}

/// The sections of a rulebook an agent judges by, each from its `## ` heading to the next, named the same
/// way; a heading the rulebook no longer carries is a broken prompt.
pub fn rulebook_sections(trees: &Trees, name: &str, headings: &[String]) -> Result<String, Refusal> {
  let text = read_required(&trees.rulebook_path(name))?;
  let sections = headings
    .iter()
    .map(|heading| {
      let start = text
        .find(&format!("\n## {heading}\n"))
        .ok_or_else(|| fault(format!("prompts/rules/{name}.md has no section \"{heading}\"")))?;
      let end = text[start + 1..].find("\n## ").map_or(text.len(), |at| at + start + 1);

      Ok(text[start + 1..end].trim().to_owned())
    })
    .collect::<Result<Vec<_>, Refusal>>()?;

  Ok(format!(
    "RULEBOOK prompts/rules/{name}.md (sections: {}):\n{}",
    headings.join(", "),
    sections.join("\n\n")
  ))
}

// The tool host's namespace as a configuration names it: its address, and the header handing the run's
// token, read from the program's own environment.
fn server_on(port: u16, namespace: &str) -> Ordered {
  Ordered::map([
    ("type", Ordered::text("http")),
    ("url", Ordered::text(format!("http://127.0.0.1:{port}/mcp/{namespace}"))),
    (
      "headers",
      Ordered::map([("Authorization", Ordered::text(format!("Bearer ${{{TOKEN_VARIABLE}}}")))]),
    ),
  ])
}

/// A line's servers on the tool host: the namespaces it uses, then the toolbook, which reaches what those
/// namespaces do not list.
pub(crate) fn line_servers(ground: &Ground, namespaces: &[&str]) -> Ordered {
  servers_on(ground.roots.mcp_port, namespaces)
}

/// The same servers on the tool host listening on the port given, each behind the run's token.
pub fn servers_on(port: u16, namespaces: &[&str]) -> Ordered {
  Ordered::map(
    namespaces
      .iter()
      .copied()
      .chain([harness_providers::toolbook::SERVER])
      .map(|namespace| (namespace, server_on(port, namespace))),
  )
}

// A configuration written where the line reads it when nothing has, and left as it stands otherwise: two
// trees writing one file would move the line's prefix back and forth between them.
pub(crate) fn handed_config(
  path: PathBuf,
  compose: impl FnOnce() -> Result<Ordered, Refusal>,
) -> Result<PathBuf, Refusal> {
  if !path.exists() {
    publish_text(&path, &Ordered::map([("mcpServers", compose()?)]).pretty())?;
  }

  Ok(path)
}

/// A search run's standing instructions: the task with `tools`, the site tools the web namespace lists,
/// then the search rulebooks — the ones binding only some searches by the section deciding whether each
/// applies — and the plain-writing and self-preservation rulebooks.
pub fn search_instructions(trees: &Trees, tools: &str) -> Result<String, Refusal> {
  let search = &RESEARCH.search;
  let task = render(&read_required(&trees.agent_dir("search").join("task.md"))?, &[("tools", tools)]);
  let mut parts = vec![task];

  for name in &search.rulebooks {
    parts.push(if search.on_request.contains(name) {
      rulebook_sections(trees, name, std::slice::from_ref(&search.applies_section))?
    } else {
      rulebook(trees, name)?
    });
  }

  parts.push(rulebook(trees, PLAIN_WRITING)?);
  parts.push(rulebook(trees, SELF_PRESERVATION)?);

  Ok(parts.join("\n\n"))
}

/// A research branch's standing instructions, with the same site tools the search itself is shown, so a
/// branch is never told a shorter list.
pub fn branch_instructions(trees: &Trees, tools: &str) -> Result<String, Refusal> {
  Ok(
    [
      render(&read_required(&trees.agent_dir("search").join("branch.md"))?, &[("tools", tools)]),
      rulebook(trees, SELF_PRESERVATION)?,
    ]
    .join("\n\n"),
  )
}

impl Runner for Search {
  fn name(&self) -> &'static str {
    "search"
  }

  fn tools_allowed(&self) -> &'static str {
    WEB_TOOLS
  }

  // The task with the site tools the web namespace lists, the search rulebooks — the ones binding only some
  // searches by the section deciding whether each applies — and the plain-writing and self-preservation
  // rulebooks. The listing is the tool host's; with none to be had the line is not composed.
  async fn instructions(&self, ground: &Ground, trees: &Trees) -> Result<Option<String>, Refusal> {
    let Some(tools) = listing(ground, "web").await else { return Ok(None) };

    search_instructions(trees, &tools).map(Some)
  }

  // The web namespace alone, on the tool host, with the toolbook.
  fn mcp_config(&self, ground: &Ground, trees: &Trees) -> Result<PathBuf, Refusal> {
    handed_config(trees.agents.join("search.mcp.json"), || Ok(line_servers(ground, &["web"])))
  }
}

impl Runner for Browse {
  fn name(&self) -> &'static str {
    "browse"
  }

  fn tools_allowed(&self) -> &'static str {
    "mcp__chrome-devtools,mcp__web,mcp__toolbook"
  }

  async fn instructions(&self, _ground: &Ground, trees: &Trees) -> Result<Option<String>, Refusal> {
    Ok(Some(
      [
        read_required(&trees.agent_dir("browse").join("task.md"))?,
        rulebook(trees, SELF_PRESERVATION)?,
      ]
      .join("\n\n"),
    ))
  }

  // The browse agent's servers as a run on the headless strategy is handed them.
  fn mcp_config(&self, ground: &Ground, trees: &Trees) -> Result<PathBuf, Refusal> {
    handed_config(trees.agents.join("browse").join(".mcp.json"), || browse_servers(ground, trees, None))
  }
}

/// A private display a browse run's Chrome is shown on: its name, its authority file and the runtime
/// directory a program on it is handed.
pub struct Shown<'a> {
  pub display: &'a str,
  pub xauthority: &'a std::path::Path,
  pub runtime_dir: &'a std::path::Path,
}

/// The browse agent's servers: its own chrome-devtools server, started apart from a person's session — no
/// keyring, the X11 platform, every variable of the desktop session emptied and its bus switched off —
/// headless unless a private display is given, which it is then shown instead; and the web, on the tool
/// host.
pub fn browse_servers(
  ground: &Ground,
  trees: &Trees,
  shown: Option<&Shown<'_>>,
) -> Result<Ordered, Refusal> {
  let own_path = trees.agent_dir("browse").join(".mcp.json");
  let own: Ordered = serde_json::from_str(
    &std::fs::read_to_string(&own_path).map_err(|why| fault(format!("{}: {why}", own_path.display())))?,
  )
  .map_err(|why| fault(format!("{}: {why}", own_path.display())))?;
  let mut servers = own
    .entries()
    .and_then(|file| file.get("mcpServers"))
    .and_then(Ordered::entries)
    .cloned()
    .unwrap_or_default();

  if let Some(Ordered::Map(chrome)) = servers.get("chrome-devtools").cloned() {
    let headless = Ordered::text("--headless");
    let mut args: Vec<Ordered> = match chrome.get("args") {
      Some(Ordered::List(args)) => {
        args.iter().filter(|arg| shown.is_none() || **arg != headless).cloned().collect()
      }
      _ => vec![],
    };
    let mut env = match chrome.get("env") {
      Some(Ordered::Map(env)) => env.clone(),
      _ => Default::default(),
    };
    let mut started = chrome;

    for arg in &BROWSER.detached_arguments {
      let arg = Ordered::text(format!("--chromeArg={arg}"));

      if !args.contains(&arg) {
        args.push(arg);
      }
    }

    for name in &BROWSER.session_variables {
      env.insert(name.clone(), Ordered::text(""));
    }

    env.insert("DBUS_SESSION_BUS_ADDRESS".to_owned(), Ordered::text("disabled:"));

    if let Some(shown) = shown {
      env.insert("DISPLAY".to_owned(), Ordered::text(shown.display));
      env.insert("XAUTHORITY".to_owned(), Ordered::text(shown.xauthority.to_string_lossy()));
      env.insert("XDG_RUNTIME_DIR".to_owned(), Ordered::text(shown.runtime_dir.to_string_lossy()));
    }

    started.insert("args".to_owned(), Ordered::List(args));
    started.insert("env".to_owned(), Ordered::Map(env));
    servers.insert("chrome-devtools".to_owned(), Ordered::Map(started));
  }

  for (name, server) in line_servers(ground, &["web"]).entries().into_iter().flatten() {
    servers.insert(name.clone(), server.clone());
  }

  Ok(Ordered::Map(servers))
}

/// A runner's run as it goes out on Claude Code: its instructions and configuration as above, the tier its
/// key resolves to there, and the effort and thinking its agent's own choice makes of that; none where its
/// instructions cannot be had now or its key resolves to nothing.
pub async fn runner_run<R: Runner>(
  ground: &Ground,
  trees: &Trees,
  runner: &R,
) -> Result<Option<RunnerRun>, Refusal> {
  let name = runner.name();
  let Some(instructions) = runner.instructions(ground, trees).await? else {
    return Ok(None);
  };
  let mapping = match tiers::resolve_execution(&ground.db, name, PROVIDER, None).await {
    Ok(mapping) => mapping,
    Err(why) => {
      eprintln!("{name}: the line resolves to no model — {why}");
      return Ok(None);
    }
  };
  let reasoning = reasoning_of(ground, name).await?;
  let effort = effort_of(
    ground,
    &ground.machine(),
    PROVIDER,
    &mapping.model,
    mapping.effort.as_deref(),
    Some(&reasoning),
  )
  .await?
  .unwrap_or_default();
  let on = On {
    model: mapping.model,
    effort,
    tier: mapping.tier,
    thinking: reasoning.thinking.unwrap_or(true),
  };

  Ok(Some(runner_run_on(ground, trees, runner, &instructions, on).await?))
}

/// The model a runner's run goes out on, its effort and tier, and whether it thinks.
pub struct On {
  pub model: String,
  pub effort: String,
  pub tier: String,
  pub thinking: bool,
}

/// A runner's run composed on the model given rather than the one its key resolves to — where a run is kept
/// on the registration that set it off — its instructions as `runner_run` reads them.
pub async fn runner_run_on<R: Runner>(
  ground: &Ground,
  trees: &Trees,
  runner: &R,
  instructions: &str,
  on: On,
) -> Result<RunnerRun, Refusal> {
  let name = runner.name();

  Ok(RunnerRun {
    line: name,
    model: on.model,
    effort: on.effort,
    tier: on.tier,
    system_prompt: system_prompt_file(trees, name, instructions)?,
    mcp_config: runner.mcp_config(ground, trees)?,
    tools_allowed: runner.tools_allowed(),
    thinking: on.thinking,
    cwd: trees.agents.join(name),
  })
}

/// The search line's tools as one run of it is handed them, told what is named: its standing configuration,
/// with what its web server is told handed back for the run's own environment.
pub fn search_tools(
  ground: &Ground,
  trees: &Trees,
  told: &[(&str, String)],
) -> Result<(PathBuf, Vec<(String, String)>), Refusal> {
  Ok((
    Search.mcp_config(ground, trees)?,
    told.iter().map(|(name, value)| ((*name).to_owned(), value.clone())).collect(),
  ))
}

/// The research branch line as every branch
/// is handed it: the branch instructions with the site tools the web namespace lists, the search line's
/// tools and its standing configuration and thinking on, on the model, effort and tier
/// given. What its web server is told goes in the run's own environment, which is handed back. None where
/// its instructions cannot be had now.
pub async fn branch_line_run(
  ground: &Ground,
  trees: &Trees,
  mapped: (&str, &str, &str),
  told: &[(&str, String)],
) -> Result<Option<(RunnerRun, Vec<(String, String)>)>, Refusal> {
  let (model, effort, tier) = mapped;
  let Some(tools) = listing(ground, "web").await else { return Ok(None) };
  let instructions = branch_instructions(trees, &tools)?;
  let (mcp_config, env) = search_tools(ground, trees, told)?;
  let run = RunnerRun {
    line: Search.name(),
    model: model.to_owned(),
    effort: effort.to_owned(),
    tier: tier.to_owned(),
    system_prompt: system_prompt_file(trees, "search-branch", &instructions)?,
    mcp_config,
    tools_allowed: Search.tools_allowed(),
    thinking: true,
    cwd: trees.agents.join(Search.name()),
  };

  Ok(Some((run, env)))
}

impl RunnerRun {
  /// The run as Claude Code's words for it, opening the session named.
  pub fn args(&self, session: &str) -> Result<Vec<String>, Refusal> {
    super::claude::apart_args(
      &super::claude::ApartRun {
        line: self.line,
        model: &self.model,
        effort: &self.effort,
        system_prompt: Some(&self.system_prompt),
        mcp_config: Some(&self.mcp_config),
        tools_shown: "",
        tools_allowed: self.tools_allowed,
        host_commands: true,
      },
      session,
    )
  }

  /// The program it runs.
  pub fn program() -> &'static str {
    ClaudeCode::PROGRAM
  }
}
