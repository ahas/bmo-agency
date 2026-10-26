//! The translate runner (docs/launch-lines.md, 「Translate runner」): one text translated on the road the road
//! resolution answers for the `translate` key — one request through the model gateway on the registry's
//! provider, a run apart on Claude Code — under the translator role's prompt and the instructions of the
//! purpose asked.

use std::time::Duration;

use harness_providers::registered::Registered;
use serde_json::{Value, json};

use crate::control::claude_accounts::Account;
use crate::door::apart::{self, Apart, answered, new_session};
use crate::door::turn::Watch;
use crate::job_run::claude::failure::Failure;
use crate::job_run::claude::walk::{Attempt, Walking, on_accounts};
use crate::lines::claude::{ApartRun, Command, Runs, apart_args, command_env};
use crate::lines::worker::{effort_of, rulebook, system_prompt_file};
use crate::lines::{Trees, read_required, role_prompt};
use crate::places::Ground;
use crate::road::{self, Ask};

/// The work key a translation goes out on.
pub const KEY: &str = "translate";

/// The line a translation runs on.
pub const LINE: &str = "translate";

/// The role whose prompt stands first in a translation's system prompt.
pub const ROLE: &str = "translator";

// The agent folder the purposes' instructions stand in, and the folder a run apart works in.
const AGENT: &str = "translator";

// How long one translation is waited for.
const ANSWER_WITHIN: Duration = Duration::from_secs(180);

/// What a translation is for, each with its own instructions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
  /// A part of a document a person reads in the console's viewer.
  Document,
  /// A text a job worker reads.
  Worker,
  /// A worker's record the owner reads.
  Record,
}

impl Purpose {
  /// The purpose a caller names, where it is one.
  pub fn read(word: &str) -> Option<Self> {
    match word {
      "document" => Some(Self::Document),
      "worker" => Some(Self::Worker),
      "record" => Some(Self::Record),
      _ => None,
    }
  }

  fn instructions(self, trees: &Trees) -> Result<String, String> {
    let read = |name: &str| read_required(&trees.agent_dir(AGENT).join(name)).map_err(|why| why.detail);

    match self {
      Self::Document => read("instructions.md"),
      Self::Worker => read("worker.md"),
      Self::Record => Ok(format!(
        "{}\n\n{}",
        rulebook(trees, "plain-writing").map_err(|why| why.detail)?,
        read("record.md")?
      )),
    }
  }

  fn word(self) -> &'static str {
    match self {
      Self::Document => "document",
      Self::Worker => "worker",
      Self::Record => "record",
    }
  }
}

/// A translation's system prompt: the translator role's prompt, then the purpose's instructions.
pub fn system_prompt(trees: &Trees, purpose: Purpose) -> Result<String, String> {
  let instructions = purpose.instructions(trees)?;

  Ok(match role_prompt(trees, ROLE) {
    Some(role) => format!("{}\n\n{instructions}", role.trim()),
    None => instructions,
  })
}

/// What a translation is asked with: the target language's line, then the text.
pub fn input(language: &str, text: &str) -> String {
  format!("Translate into {}.\n\n{text}", language.trim())
}

/// A translation as it came back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Translated {
  pub text: String,
  pub provider: String,
  pub model: String,
}

/// Why a translation has none.
#[derive(Debug)]
pub enum Refused {
  Ask(String),
  Closed(String),
  NoAnswer(String),
}

/// One text translated on the `translate` road.
pub async fn translate(
  ground: &Ground,
  purpose: &str,
  language: &str,
  text: &str,
) -> Result<Translated, Refused> {
  let purpose = Purpose::read(purpose)
    .ok_or_else(|| Refused::Ask(format!("{purpose} is no purpose: document, worker or record")))?;

  if language.trim().is_empty() || text.trim().is_empty() {
    return Err(Refused::Ask("a translation names its language and its text".to_owned()));
  }

  let resolution = road::resolve(ground, &Ask::key(KEY)).await.map_err(Refused::NoAnswer)?;
  let Some(taken) = resolution.road().cloned() else {
    road::went(ground, &resolution, None, &[]).await;

    return Err(Refused::Closed(resolution.sentence()));
  };

  road::went(ground, &resolution, Some(&taken), &[]).await;

  let trees = Trees::of(ground);
  let system = system_prompt(&trees, purpose).map_err(Refused::NoAnswer)?;
  let input = input(language, text);
  let answer = if taken.provider == crate::control::registry::PROVIDER {
    on_gateway(ground, &taken.model, &system, &input).await
  } else if taken.provider == crate::lines::runner::PROVIDER {
    apart_on_claude(ground, &trees, purpose, &taken.model, &system, &input).await
  } else {
    Err(format!("{} carries no translation", taken.provider))
  }
  .map_err(Refused::NoAnswer)?;

  Ok(Translated {
    text: answer,
    provider: taken.provider,
    model: taken.model,
  })
}

/// The request a translation is on the registry's provider: its system prompt, the input, thinking off and the
/// profile's sampling.
pub fn gateway_request(registration: &str, system: &str, input: &str) -> Value {
  let mut request = json!({
    "model": registration,
    "messages": [
      { "role": "system", "content": system },
      { "role": "user", "content": input },
    ],
    "chat_template_kwargs": { "enable_thinking": false },
  });
  let profile = policy::work_keys::WORK_KEYS.profile(KEY);

  if let Some(tokens) = profile.and_then(|profile| profile.gate_answer_tokens) {
    request["max_tokens"] = tokens.into();
  }

  for (name, value) in profile.and_then(|profile| profile.gate_sampling.as_ref()).into_iter().flatten() {
    request[name] = (*value).into();
  }

  request
}

// One request through the model gateway, answering the model's words.
async fn on_gateway(
  ground: &Ground,
  registration: &str,
  system: &str,
  input: &str,
) -> Result<String, String> {
  let machine = ground.machine();
  let url = harness_machine::gateway::url(machine.var("BMO_LLM_GATEWAY_URL"));
  let key = std::fs::read_to_string(Registered::key_file(&machine)).unwrap_or_default();
  let answer = reqwest::Client::new()
    .post(format!("{url}/v1/chat/completions"))
    .bearer_auth(key.trim())
    .json(&gateway_request(registration, system, input))
    .timeout(ANSWER_WITHIN)
    .send()
    .await
    .map_err(|why| format!("the gateway: {why}"))?;
  let status = answer.status();
  let body: Value = answer.json().await.map_err(|why| format!("the gateway's answer: {why}"))?;

  if !status.is_success() {
    return Err(format!("the gateway answered {}: {}", status.as_u16(), body["error"]));
  }

  body
    .pointer("/choices/0/message/content")
    .and_then(Value::as_str)
    .map(str::trim)
    .filter(|text| !text.is_empty())
    .map(str::to_owned)
    .ok_or_else(|| "the gateway answered no words".to_owned())
}

// A run apart's pane watched for nothing but its end.
struct Quiet;

impl Watch for Quiet {}

// One translation on one login, a run apart of its own.
struct OnLogin<'a> {
  ground: &'a Ground,
  trees: &'a Trees,
  purpose: Purpose,
  model: &'a str,
  effort: String,
  system: std::path::PathBuf,
  input: &'a str,
}

impl Attempt<String> for OnLogin<'_> {
  async fn attempt(&mut self, account: Option<Account>) -> Result<String, Failure> {
    let session = new_session();
    let args = apart_args(
      &ApartRun {
        line: LINE,
        model: self.model,
        effort: &self.effort,
        system_prompt: Some(&self.system),
        mcp_config: None,
        tools_shown: "",
        tools_allowed: "",
        host_commands: false,
      },
      &session,
    )
    .map_err(|why| Failure::error(why.detail))?;
    let env = command_env(self.ground.env(), account.as_ref(), &Runs::Here, false)
      .map_err(|why| Failure::error(why.detail))?;
    let cwd = self.trees.agents.join(AGENT);

    std::fs::create_dir_all(&cwd).map_err(|why| Failure::error(why.to_string()))?;

    let command = Command {
      args,
      cwd: cwd.to_string_lossy().into_owned(),
      env,
    };
    let ending = apart::run(
      self.ground,
      Apart {
        agent: AGENT,
        line: LINE,
        provider: crate::lines::runner::PROVIDER,
        title: format!("{KEY} · {} · {}", self.purpose.word(), self.model),
        command,
        session,
        prompt: self.input.to_owned(),
        work_key: KEY.to_owned(),
        model: self.model.to_owned(),
        effort: self.effort.clone(),
        tier: None,
        bound: Some(ANSWER_WITHIN),
        parent: None,
        root: None,
      },
      &mut Quiet,
    )
    .await
    .map_err(|why| Failure::Transport { message: why })?;

    Ok(answered(ending, KEY)?.text.trim().to_owned())
  }
}

// A run apart on Claude Code, on the logins the key's walk offers.
async fn apart_on_claude(
  ground: &Ground,
  trees: &Trees,
  purpose: Purpose,
  model: &str,
  system: &str,
  input: &str,
) -> Result<String, String> {
  let provider = crate::lines::runner::PROVIDER;
  let mapping = crate::control::tiers::resolve_execution(&ground.db, KEY, provider, None).await?;
  let effort = effort_of(ground, &ground.machine(), provider, model, mapping.effort.as_deref(), None)
    .await
    .map_err(|why| why.detail)?
    .unwrap_or_default();
  let system =
    system_prompt_file(trees, &format!("{LINE}-{}", purpose.word()), system).map_err(|why| why.detail)?;
  let mut attempt = OnLogin {
    ground,
    trees,
    purpose,
    model,
    effort,
    system,
    input,
  };
  let text = on_accounts(&Walking { ground }, KEY, None, &mut attempt)
    .await
    .map_err(|failure| failure.message())?;

  Some(text)
    .filter(|text| !text.is_empty())
    .ok_or_else(|| "the run answered no words".to_owned())
}
