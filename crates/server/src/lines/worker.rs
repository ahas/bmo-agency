//! The job worker's line (docs/launch-lines.md; docs/agent-control.md, 「A worker standing with nothing
//! to do」): its model and effort as the job key's tier maps them, the worker's own thinking and level,
//! its system prompt where the account it runs on names a base prompt or it chooses one, the standing
//! rules appended to the program's own prompt otherwise, the MCP configuration, the settings, the plugin
//! and the co-worker definitions it is handed.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use harness_providers::Machine;
use policy::base_prompts::BASE_PROMPTS;
use policy::reasoning::REASONING;
use policy::run_lines::RUN_LINES;
use regex::Regex;
use serde::Serialize;
use serde::ser::SerializeMap;
use sha2::{Digest, Sha256};

use super::{Trees, fault, publish_text, read_required};
use crate::Refusal;
use crate::control::claude_accounts::Account;
use crate::control::harness::{levels, named};
use crate::control::tiers;
use crate::places::Ground;
use crate::queue::routes::JOB_WORK_KEY;
use store::{all, one};

/// The line's name.
pub const LINE: &str = "worker";

// The rulebook every worker's rules carry after the standing rules.
pub const SELF_PRESERVATION: &str = "self-preservation";

// A value a base prompt names, as `{{name}}`: the base prompts' own syntax.
static VALUE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\{\{([A-Za-z0-9_]+)\}\}").expect("the value mark"));

/// What an agent's row names of its own thinking and effort, each none where the tier and the line
/// decide.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reasoning {
  pub thinking: Option<bool>,
  pub level: Option<String>,
}

/// One worker's run on its line, composed before any road is chosen.
#[derive(Clone, Debug)]
pub struct WorkerRun {
  pub model: String,
  pub effort: String,
  pub tier: Option<String>,
  pub thinking: bool,
  /// The worker's own choice, which a mapping laid on the run reads; none for a run that is no
  /// worker's.
  pub reasoning: Option<Reasoning>,
  pub system_prompt: Option<PathBuf>,
  pub rules: PathBuf,
  pub mcp_config: PathBuf,
  pub settings: PathBuf,
  pub plugin_dir: PathBuf,
  pub agents_json: String,
  pub tools_denied: String,
  pub cwd: String,
}

/// The name a worker's own documents go by: its own, or its base's for a support worker.
pub async fn documents_of(ground: &Ground, worker: &str) -> Result<String, Refusal> {
  let row = one(
    &ground.db,
    r#"SELECT "support_of" FROM "agents" WHERE "name" = $1"#,
    vec![worker.into()],
  )
  .await?;

  Ok(
    row
      .and_then(|row| row.try_get::<Option<String>>("", "support_of").ok().flatten())
      .unwrap_or_else(|| worker.to_owned()),
  )
}

/// What the row of the agent's own choice names: a support worker runs on its base's.
pub async fn reasoning_of(ground: &Ground, agent: &str) -> Result<Reasoning, Refusal> {
  let sql = r#"SELECT
                 COALESCE(b."thinking", CASE WHEN a."support_of" IS NULL THEN a."thinking" END) AS "thinking",
                 CASE WHEN a."support_of" IS NULL THEN a."effort" ELSE b."effort" END AS "effort",
                 a."support_of" IS NOT NULL AND b."name" IS NULL AS "orphan"
               FROM "agents" a LEFT JOIN "agents" b ON b."name" = a."support_of" WHERE a."name" = $1"#;
  let Some(row) = one(&ground.db, sql, vec![agent.into()]).await? else {
    return Ok(Reasoning::default());
  };

  if row.try_get::<bool>("", "orphan")? {
    return Ok(Reasoning::default());
  }

  let thinking: Option<i64> = row.try_get("", "thinking")?;
  let level: Option<String> = row.try_get("", "effort")?;

  Ok(Reasoning {
    thinking: thinking.map(|value| value != 0),
    level: level.filter(|level| REASONING.holds(level)),
  })
}

/// A worker's rules: those its co-workers share — every worker's rules and the worker's own after them —
/// and the worker's whole, which carry its prefix documents after those, written as one file: one shared
/// by every worker with nothing of its own and one for each worker with something.
pub struct StandingRules {
  pub shared: String,
  pub text: String,
  pub file: PathBuf,
}

/// The report trailer every job is held to, its graph text, the worker plugin's skills and the plain-writing
/// rulebook filled in as the deployment shows them (docs/launch-lines.md): `shown` is the code root by the
/// deployment's link, the documents being read out of `trees`.
pub fn job_trailer(trees: &Trees, shown: &Trees) -> Result<String, Refusal> {
  let documents = trees.agent_dir(LINE);
  let graph = read_required(&documents.join("graph.md"))?;
  let skills = shown.agent_dir(LINE).join("plugin").join("skills").to_string_lossy().into_owned();
  let plain = shown.rulebook_path("plain-writing").to_string_lossy().into_owned();

  Ok(super::render(
    &read_required(&documents.join("trailer.md"))?,
    &[("graph", graph.trim()), ("skills", &skills), ("plainWriting", &plain)],
  ))
}

/// Every worker's rules, the worker's own after them, the job's report trailer after those where the rules
/// are a worker's, and the documents its row names for its prefix last, as one file: the session's cached
/// prefix, paid for once.
pub fn standing_rules(
  trees: &Trees,
  named: Option<&str>,
  worker: Option<&str>,
  trailer: Option<&str>,
  documents: Option<&str>,
) -> Result<StandingRules, Refusal> {
  let own_path = named.map(|named| trees.agent_dir(named).join("own-rules.md"));
  let own = match &own_path {
    Some(path) if path.exists() => Some(read_required(path)?),
    _ => None,
  };
  let mut parts = vec![
    read_required(&trees.agent_dir(LINE).join("standing-rules.md"))?,
    rulebook(trees, SELF_PRESERVATION)?,
  ];

  parts.extend(own.clone());

  let shared = parts.join("\n\n");

  parts.extend(trailer.map(str::to_owned));
  parts.extend(documents.map(str::to_owned));

  let text = parts.join("\n\n");
  let name = match (documents, worker, own, named) {
    (Some(_), Some(worker), _, _) => format!("worker-rules-{worker}"),
    (_, _, Some(_), Some(named)) => format!("worker-rules-{named}"),
    _ => "worker-rules".to_owned(),
  };
  let file = system_prompt_file(trees, &name, &text)?;

  Ok(StandingRules { shared, text, file })
}

/// The documents a worker's row names for its session's prefix (docs/worker-bench.md, 「Prefix
/// documents」), read fresh in the list's order, each headed by its path as written: a relative path from
/// the worker's checkout, an absolute one where it stands. A support worker with none of its own takes its
/// base's. A document that cannot be read is left out and said in the log; none where the list is empty.
pub async fn prefix_documents(ground: &Ground, worker: &str, cwd: &str) -> Result<Option<String>, Refusal> {
  let row = one(
    &ground.db,
    r#"SELECT COALESCE(own."prefix_documents", base."prefix_documents") AS "documents"
       FROM "agents" own LEFT JOIN "agents" base ON base."name" = own."support_of"
       WHERE own."name" = $1"#,
    vec![worker.into()],
  )
  .await?;
  let listed: Vec<String> = row
    .and_then(|row| row.try_get::<Option<String>>("", "documents").ok().flatten())
    .and_then(|text| serde_json::from_str(&text).ok())
    .unwrap_or_default();
  let mut read = Vec::new();

  for path in &listed {
    let at = Path::new(cwd).join(path);

    match std::fs::read_to_string(&at) {
      Ok(text) => read.push(format!("DOCUMENT {path}:\n{}", text.trim_end())),
      Err(why) => eprintln!("worker {worker}: its prefix document {} was left out: {why}", at.display()),
    }
  }

  Ok((!read.is_empty()).then(|| read.join("\n\n")))
}

/// A rulebook as an agent is given it, named so the agent can say which rule it applied.
pub fn rulebook(trees: &Trees, name: &str) -> Result<String, Refusal> {
  Ok(format!(
    "RULEBOOK prompts/rules/{name}.md:\n{}",
    read_required(&trees.rulebook_path(name))?
  ))
}

/// A composed prompt written where a line hands it over, under the agents root.
pub fn system_prompt_file(trees: &Trees, name: &str, text: &str) -> Result<PathBuf, Refusal> {
  let path = trees.agents.join("prompts").join(format!("{name}.md"));

  publish_text(&path, text)?;

  Ok(path)
}

/// How this side reads each value a base prompt may name, for the run it is composed for.
fn base_prompt_value(ground: &Ground, name: &str, cwd: &str, model: &str) -> Option<String> {
  Some(match name {
    "cwd" => cwd.to_owned(),
    "date" => chrono::Local::now().format("%Y-%m-%d").to_string(),
    "platform" => std::env::consts::OS.to_owned(),
    "os_version" => os_version(),
    "shell" => Path::new(ground.vars().get("SHELL").map_or("sh", String::as_str))
      .file_name()
      .map_or_else(|| "sh".to_owned(), |name| name.to_string_lossy().into_owned()),
    "model" => model.to_owned(),
    _ => return None,
  })
}

// The operating system's name and release, as the kernel answers them.
fn os_version() -> String {
  // SAFETY: `uname` fills the zeroed structure it is handed and nothing else.
  let mut name: libc::utsname = unsafe { std::mem::zeroed() };

  // SAFETY: as above; the fields it fills are NUL-terminated.
  if unsafe { libc::uname(&mut name) } != 0 {
    return String::new();
  }

  let field = |chars: &[libc::c_char]| {
    // SAFETY: `uname` NUL-terminates each field within its array.
    unsafe { std::ffi::CStr::from_ptr(chars.as_ptr()) }.to_string_lossy().into_owned()
  };

  format!("{} {}", field(&name.sysname), field(&name.release))
}

/// A base prompt by name, filled in for the run, or none where the store keeps no prompt of that name
/// (docs/base-prompts.md).
pub async fn base_prompt_text(
  ground: &Ground,
  prompt: &str,
  cwd: &str,
  model: &str,
) -> Result<Option<String>, Refusal> {
  let sql = r#"SELECT "text" FROM "base_prompts" WHERE "name" = $1"#;
  let Some(row) = one(&ground.db, sql, vec![prompt.into()]).await? else {
    return Ok(None);
  };
  let text: String = row.try_get("", "text")?;
  let filled = VALUE.replace_all(&text, |found: &regex::Captures| {
    let name = &found[1];

    BASE_PROMPTS
      .values
      .iter()
      .any(|one| one == name)
      .then(|| base_prompt_value(ground, name, cwd, model))
      .flatten()
      .unwrap_or_else(|| found[0].to_owned())
  });

  Ok(Some(filled.trim().to_owned()))
}

/// The prompt the agent chooses, filled in for the run, or none where it chooses none
/// (docs/base-prompts.md).
pub async fn chosen_base_prompt(
  ground: &Ground,
  agent: &str,
  cwd: &str,
  model: &str,
) -> Result<Option<String>, Refusal> {
  let sql = r#"SELECT "prompt" FROM "agent_base_prompts" WHERE "agent" = $1"#;
  let Some(row) = one(&ground.db, sql, vec![agent.into()]).await? else {
    return Ok(None);
  };
  let prompt: String = row.try_get("", "prompt")?;

  base_prompt_text(ground, &prompt, cwd, model).await
}

/// The worker's whole system prompt where one stands where the program's own would: the base prompt the
/// account names, which wins over the agent's own choice, or the prompt the agent chooses, filled in for
/// the run, then the worker's rules, as one file named by what it holds (docs/base-prompts.md).
#[allow(clippy::too_many_arguments)]
pub async fn worker_system_prompt(
  ground: &Ground,
  trees: &Trees,
  named: Option<&str>,
  account: Option<&Account>,
  rules: &str,
  above: Option<&str>,
  cwd: &str,
  model: &str,
) -> Result<Option<PathBuf>, Refusal> {
  let (base, holder) =
    match account.and_then(|account| account.base_prompt.as_deref().map(|prompt| (account, prompt))) {
      Some((account, prompt)) => {
        let model = account.base_model(model);
        let Some(base) = base_prompt_text(ground, prompt, cwd, &model).await? else {
          return Ok(None);
        };

        (base, format!("account-{}", account.id))
      }
      None => {
        let Some(named) = named else { return Ok(None) };
        let Some(base) = chosen_base_prompt(ground, named, cwd, model).await? else {
          return Ok(None);
        };

        (base, named.to_owned())
      }
    };
  let text = match above {
    Some(above) => format!("{above}\n\n{base}\n\n{rules}"),
    None => format!("{base}\n\n{rules}"),
  };
  let digest = format!("{:x}", Sha256::digest(text.as_bytes()));

  Ok(Some(system_prompt_file(
    trees,
    &format!("worker-system-{holder}-{}", &digest[..12]),
    &text,
  )?))
}

/// One co-worker definition as the program reads it, its fields in the order the line writes them.
#[derive(Serialize)]
struct Definition {
  description: String,
  prompt: String,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  tools: Vec<String>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  skills: Vec<String>,
  model: String,
  effort: String,
}

// The definitions in the order the archive lists the roles.
struct Definitions(Vec<(String, Definition)>);

impl Serialize for Definitions {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(self.0.len()))?;

    for (name, definition) in &self.0 {
      map.serialize_entry(name, definition)?;
    }

    map.end()
  }
}

// The name the program files the plugin's skills under, and the skills it carries.
fn plugin_skills(plugin_dir: &Path) -> Result<Vec<String>, Refusal> {
  let manifest = read_required(&plugin_dir.join(".claude-plugin").join("plugin.json"))?;
  let name = serde_json::from_str::<serde_json::Value>(&manifest)
    .ok()
    .and_then(|manifest| manifest.get("name").and_then(serde_json::Value::as_str).map(str::to_owned))
    .ok_or_else(|| fault(format!("{}: the plugin names no name", plugin_dir.display())))?;
  let skills = std::fs::read_dir(plugin_dir.join("skills"))
    .map_err(|why| fault(format!("{}: {why}", plugin_dir.display())))?;

  Ok(
    skills
      .filter_map(Result::ok)
      .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
      .map(|entry| format!("{name}:{}", entry.file_name().to_string_lossy()))
      .collect(),
  )
}

/// The co-worker definitions a job is handed whole: every enabled role, on the model and effort its
/// own key maps to on Claude Code, its prompt closed by the worker's rules, and only the skills the
/// plugin carries.
pub async fn coworkers_json(ground: &Ground, plugin_dir: &Path, rules: &str) -> Result<String, Refusal> {
  let carried = plugin_skills(plugin_dir)?;
  let rows = all(
    &ground.db,
    r#"SELECT "name", "description", "prompt", "tools", "skills" FROM "coworker_pool_roles"
       WHERE "enabled" = 1 ORDER BY "name""#,
    vec![],
  )
  .await?;
  let list = |text: String| serde_json::from_str::<Vec<String>>(&text).unwrap_or_default();
  let mut definitions = Vec::with_capacity(rows.len());

  for row in rows {
    let name: String = row.try_get("", "name")?;
    let prompt: String = row.try_get("", "prompt")?;
    let mapping = tiers::resolve(&ground.db, &format!("role:{name}"), "claude-code")
      .await
      .map_err(fault)?;

    definitions.push((
      name,
      Definition {
        description: row.try_get("", "description")?,
        prompt: format!("{}\n\n{rules}", prompt.trim()),
        tools: list(row.try_get("", "tools")?),
        skills: list(row.try_get("", "skills")?)
          .into_iter()
          .filter(|skill| carried.contains(skill))
          .collect(),
        model: mapping.model,
        effort: mapping.effort.unwrap_or_default(),
      },
    ));
  }

  serde_json::to_string(&Definitions(definitions)).map_err(|why| fault(why.to_string()))
}

/// The worker line's MCP configuration, on the tool host — the web namespace and the worker's own — written
/// where the line reads it when nothing has written it, and left as it stands otherwise: two trees writing
/// one file would move the worker's prefix back and forth between them. A hosted tool those namespaces do
/// not show is reached with `bmo use`.
pub fn mcp_config(ground: &Ground, trees: &Trees) -> Result<PathBuf, Refusal> {
  super::runner::handed_config(trees.agents.join("worker.mcp.json"), || {
    Ok(super::runner::line_servers(ground, &["web", "worker"]))
  })
}

/// The worker's run on its line: `worker` names the worker whose own documents, thinking, level and
/// base prompt it carries, none for a run that is no worker's in particular, and `account` the login it
/// goes out on, whose base prompt stands where the line's own choice would.
pub async fn worker_run(
  ground: &Ground,
  trees: &Trees,
  worker: Option<&str>,
  account: Option<&Account>,
  model: &str,
  cwd: &str,
) -> Result<WorkerRun, Refusal> {
  let named = match worker {
    Some(worker) => Some(documents_of(ground, worker).await?),
    None => None,
  };
  let documents = match worker {
    Some(worker) => prefix_documents(ground, worker, cwd).await?,
    None => None,
  };
  let shown = Trees {
    code: ground.shown_code_root(),
    ..trees.clone()
  };
  let trailer = match worker {
    Some(_) => Some(job_trailer(trees, &shown)?),
    None => None,
  };
  let StandingRules {
    shared,
    text: rules_text,
    file: rules,
  } = standing_rules(trees, named.as_deref(), worker, trailer.as_deref(), documents.as_deref())?;
  let above = match worker {
    Some(worker) => crate::conversational::role_of(&ground.db, worker)
      .await
      .ok()
      .flatten()
      .and_then(|role| super::role_prompt(trees, &role)),
    None => None,
  };
  let system_prompt = worker_system_prompt(
    ground,
    trees,
    named.as_deref(),
    account,
    &rules_text,
    above.as_deref(),
    cwd,
    model,
  )
  .await?;
  let worker_dir = trees.agent_dir(LINE);
  let plugin_dir = worker_dir.join("plugin");
  let reasoning = match worker {
    Some(worker) => Some(reasoning_of(ground, worker).await?),
    None => None,
  };

  Ok(WorkerRun {
    model: model.to_owned(),
    effort: "high".to_owned(),
    tier: None,
    thinking: reasoning.as_ref().and_then(|reasoning| reasoning.thinking).unwrap_or(true),
    reasoning,
    system_prompt,
    rules,
    mcp_config: mcp_config(ground, trees)?,
    settings: worker_dir.join("settings.json"),
    agents_json: coworkers_json(ground, &plugin_dir, &shared).await?,
    plugin_dir,
    tools_denied: RUN_LINES.line(LINE).map(|line| line.tools_denied.join(",")).unwrap_or_default(),
    cwd: cwd.to_owned(),
  })
}

/// The run with the job key's tier mapping on a provider laid on it: the mapping's model and tier,
/// and the effort word the provider makes of the mapping's word and the worker's own choice.
pub async fn configured(
  ground: &Ground,
  machine: &Machine,
  run: WorkerRun,
  provider: &str,
) -> Result<WorkerRun, Refusal> {
  configured_for(ground, machine, run, JOB_WORK_KEY, provider).await
}

/// The run with a work key's tier mapping on a provider laid on it, as a job's run takes the job key's.
pub async fn configured_for(
  ground: &Ground,
  machine: &Machine,
  mut run: WorkerRun,
  key: &str,
  provider: &str,
) -> Result<WorkerRun, Refusal> {
  let mapping = tiers::resolve_execution(&ground.db, key, provider, None).await.map_err(fault)?;

  run.effort = effort_of(
    ground,
    machine,
    provider,
    &mapping.model,
    mapping.effort.as_deref(),
    run.reasoning.as_ref(),
  )
  .await?
  .unwrap_or_default();
  run.model = mapping.model;
  run.tier = Some(mapping.tier);

  Ok(run)
}

/// The effort word a run of a mapping goes out with where an agent's choice reaches it: the provider's
/// own answer, and the mapping's word where no choice does.
pub async fn effort_of(
  ground: &Ground,
  machine: &Machine,
  provider: &str,
  model: &str,
  mapped: Option<&str>,
  reasoning: Option<&Reasoning>,
) -> Result<Option<String>, Refusal> {
  let Some(reasoning) = reasoning else { return Ok(mapped.map(str::to_owned)) };
  let provider = named(provider).map_err(|refused| fault(refused.error))?;
  let levels = levels(crate::control::claude_accounts::Judging::of(ground), machine, provider, model).await;

  Ok(provider.run_effort(mapped, reasoning.thinking, reasoning.level.as_deref(), &levels))
}
