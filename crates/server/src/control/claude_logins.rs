//! A Claude account signed in from the console (docs/cache-fallback.md, 「Signing an account in from the
//! console」): the CLI's own sign-in started with the account's configuration directory as its own, the address
//! it prints handed back, and the code the owner pastes handed to it. The CLI makes the login; nothing of the
//! exchange, and never the code, is kept, logged or answered.

use std::collections::HashMap;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use policy::providers::claude_code::CLAUDE_CODE;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;

use super::claude_account_changes::{self as changes, BasePromptGiven, CredentialGiven};
use super::claude_accounts::{CREDENTIAL_FILE, Credential, CredentialKind, Judging, accounts};
use super::fallback::accounts_answer;
use super::{Call, Refused, Route, answer};

/// The variables a sign-in's CLI is never handed: a login the launching process carries would answer for the
/// sign-in in place of the one being made.
const CARRIED_LOGINS: [&str; 4] = [
  "CLAUDE_CODE_OAUTH_TOKEN",
  "ANTHROPIC_API_KEY",
  "ANTHROPIC_AUTH_TOKEN",
  "CLAUDE_CONFIG_DIR",
];

/// What a sign-in is for: an account there, signed in again in its own directory, or one added once its
/// directory holds a login.
enum Target {
  Again { home: PathBuf },
  New { name: String, home: PathBuf },
}

impl Target {
  fn home(&self) -> &Path {
    match self {
      Self::Again { home } | Self::New { home, .. } => home,
    }
  }
}

/// One sign-in under way: the CLI running it, what it has said so far, and what it is for. The CLI is ended
/// when the sign-in is dropped.
struct SignIn {
  cli: Child,
  input: Option<ChildStdin>,
  said: Arc<std::sync::Mutex<String>>,
  target: Target,
  started: Instant,
}

static SIGN_INS: LazyLock<Mutex<HashMap<String, SignIn>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn words() -> &'static policy::control_words::LoginsWords {
  &CONTROL_WORDS.logins
}

// What the CLI writes, kept as it comes, so the address and a refusal are read from it.
fn listen(mut from: impl AsyncRead + Unpin + Send + 'static, said: Arc<std::sync::Mutex<String>>) {
  tokio::spawn(async move {
    let mut chunk = [0u8; 4096];
    while let Ok(read) = from.read(&mut chunk).await {
      if read == 0 {
        break;
      }
      said
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push_str(&String::from_utf8_lossy(&chunk[..read]));
    }
  });
}

/// What the CLI said as a person reads it: its terminal escapes, the hyperlinks among them, taken out.
pub fn plain(said: &str) -> String {
  String::from_utf8_lossy(&strip_ansi_escapes::strip(said.as_bytes())).into_owned()
}

/// The sign-in's address among what the CLI said: the first word of it that reads as an https address. The
/// CLI's words are its own shape, no general format, and the address is checked by the URL reader.
pub fn sign_in_address(said: &str) -> Option<String> {
  plain(said)
    .split_whitespace()
    .filter_map(|word| word.find("https://").map(|at| &word[at..]))
    .find_map(|word| url::Url::parse(word).ok().filter(|address| address.scheme() == "https"))
    .map(String::from)
}

/// The CLI's last words, as a refusal says them, with the code taken out wherever it stands.
pub fn last_words(said: &str, code: Option<&str>) -> String {
  let line = plain(said)
    .lines()
    .map(str::trim)
    .rev()
    .find(|line| !line.is_empty())
    .map(str::to_owned)
    .unwrap_or_else(|| words().silent.clone());
  match code.map(str::trim).filter(|code| !code.is_empty()) {
    Some(code) => line.replace(code, "…"),
    None => line,
  }
}

/// A new account's directory under the data root's env directory, named by the account's name: its letters
/// and digits, dots, underscores and hyphens, every other character a hyphen.
pub fn new_home(data_root: &Path, name: &str) -> PathBuf {
  let named: String = name
    .trim()
    .to_lowercase()
    .chars()
    .map(|letter| if letter.is_ascii_alphanumeric() || ".-_".contains(letter) { letter } else { '-' })
    .collect();
  data_root.join("env").join(named.trim_matches(|letter| ".-_".contains(letter)))
}

// Every sign-in that has stood longer than the policy allows, ended.
fn sweep(held: &mut HashMap<String, SignIn>) {
  let keep = Duration::from_secs(CLAUDE_CODE.console_login.keep_minutes * 60);
  held.retain(|_, sign_in| sign_in.started.elapsed() < keep);
}

fn text<'a>(body: &'a Value, field: &str) -> Option<&'a str> {
  body.get(field).and_then(Value::as_str).map(str::trim).filter(|text| !text.is_empty())
}

// What a start's body names: an account there, which must be a directory account, or a new one's name, whose
// directory must be empty or not yet made.
async fn target_of(db: &DatabaseConnection, data_root: &Path, body: &Value) -> Result<Target, Refused> {
  match (text(body, "account"), text(body, "name")) {
    (Some(id), None) => {
      let held = accounts(db).await?;
      let account = held.iter().find(|one| one.id == id).ok_or_else(|| {
        Refused::new(StatusCode::NOT_FOUND, fill_marks(&CONTROL_WORDS.accounts.unknown, &[("id", id)]))
      })?;
      match &account.credential {
        Credential::Directory { home } => Ok(Target::Again { home: home.clone() }),
        _ => Err(
          Refused::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            fill_marks(&words().not_directory, &[("name", &account.name)]),
          )
          .with_code("not-a-directory"),
        ),
      }
    }
    (None, Some(name)) => {
      let home = new_home(data_root, name);
      let holds = std::fs::read_dir(&home).map(|mut entries| entries.next().is_some()).unwrap_or(false);
      if holds || home == data_root.join("env") {
        return Err(
          Refused::new(
            StatusCode::CONFLICT,
            fill_marks(&words().directory_taken, &[("path", &home.to_string_lossy())]),
          )
          .with_code("directory-taken"),
        );
      }
      Ok(Target::New {
        name: name.to_owned(),
        home,
      })
    }
    _ => Err(Refused::new(StatusCode::BAD_REQUEST, words().which.clone())),
  }
}

/// `POST /claude-logins`: a sign-in started, answered with its identifier and the address to approve it at.
pub struct StartClaudeLogin;

impl Route for StartClaudeLogin {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/claude-logins";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let target = target_of(call.store(), &call.data_root(), &body).await?;
    std::fs::DirBuilder::new()
      .recursive(true)
      .mode(0o700)
      .create(target.home())
      .map_err(Refused::failed)?;

    let policy = &CLAUDE_CODE.console_login;
    let (program, arguments) =
      policy.command.split_first().ok_or_else(|| Refused::failed("no sign-in command"))?;
    let mut command = Command::new(program);
    command
      .args(arguments)
      .env("CLAUDE_CONFIG_DIR", target.home())
      // The browser is the owner's own, wherever they are: none is opened on this machine.
      .env("BROWSER", "true")
      .env_remove("DISPLAY")
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .kill_on_drop(true);
    for carried in CARRIED_LOGINS.iter().filter(|name| **name != "CLAUDE_CONFIG_DIR") {
      command.env_remove(carried);
    }
    let mut cli = command.spawn().map_err(Refused::failed)?;
    let said = Arc::new(std::sync::Mutex::new(String::new()));
    if let Some(out) = cli.stdout.take() {
      listen(out, Arc::clone(&said));
    }
    if let Some(err) = cli.stderr.take() {
      listen(err, Arc::clone(&said));
    }
    let input = cli.stdin.take();

    let bound = Instant::now() + Duration::from_secs(policy.link_seconds);
    let address = loop {
      let heard = said.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
      if let Some(address) = sign_in_address(&heard) {
        break address;
      }
      let ended = cli.try_wait().map_err(Refused::failed)?.is_some();
      if ended || Instant::now() >= bound {
        if let Target::New { home, .. } = &target {
          let _ = std::fs::remove_dir(home);
        }
        return Err(Refused::new(
          StatusCode::UNPROCESSABLE_ENTITY,
          fill_marks(&words().no_link, &[("said", &last_words(&heard, None))]),
        ));
      }
      tokio::time::sleep(Duration::from_millis(200)).await;
    };

    let login = uuid::Uuid::new_v4().to_string();
    let mut held = SIGN_INS.lock().await;
    sweep(&mut held);
    held.insert(
      login.clone(),
      SignIn {
        cli,
        input,
        said,
        target,
        started: Instant::now(),
      },
    );

    Ok(answer(StatusCode::CREATED, json!({ "login": login, "url": address })))
  }
}

crate::moved!(StartClaudeLogin);

fn no_sign_in(login: &str) -> Refused {
  Refused::new(StatusCode::NOT_FOUND, fill_marks(&words().none, &[("login", login)]))
    .with_code("no-sign-in")
}

/// `POST /claude-logins/{login}/code`: the code handed to the sign-in's CLI, the login it writes checked, and a
/// new account added once it is there.
pub struct ClaudeLoginCode;

impl Route for ClaudeLoginCode {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/claude-logins/{login}/code";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let login = call.param("login").unwrap_or_default().to_owned();
    let body = call.body().await?;
    let code = text(&body, "code").map(str::to_owned).ok_or_else(|| {
      Refused::new(StatusCode::BAD_REQUEST, words().code_needed.clone()).with_field("code")
    })?;

    // The sign-in is taken out of the held ones: it ends here, whatever the CLI makes of the code.
    let mut sign_in = {
      let mut held = SIGN_INS.lock().await;
      sweep(&mut held);
      held.remove(&login).ok_or_else(|| no_sign_in(&login))?
    };
    let refused = |said: &str| {
      Refused::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        fill_marks(&words().refused, &[("said", &last_words(said, Some(&code)))]),
      )
      .with_code("sign-in-refused")
    };
    let heard = || sign_in.said.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();

    let Some(mut input) = sign_in.input.take() else {
      return Err(refused(&heard()));
    };
    if input.write_all(format!("{code}\n").as_bytes()).await.is_err() {
      return Err(refused(&heard()));
    }
    drop(input);

    let finish = Duration::from_secs(CLAUDE_CODE.console_login.finish_seconds);
    let ended = tokio::time::timeout(finish, sign_in.cli.wait()).await;
    let well = matches!(ended, Ok(Ok(status)) if status.success());
    if !well {
      return Err(refused(&heard()));
    }

    let home = sign_in.target.home().to_path_buf();
    if !home.join(CREDENTIAL_FILE).is_file() {
      return Err(
        Refused::new(
          StatusCode::UNPROCESSABLE_ENTITY,
          fill_marks(&words().no_login, &[("path", &home.to_string_lossy())]),
        )
        .with_code("sign-in-refused"),
      );
    }

    if let Target::New { name, home } = &sign_in.target {
      let given = CredentialGiven {
        kind: Some(CredentialKind::Directory),
        path: Some(home.to_string_lossy().into_owned()),
        ..CredentialGiven::default()
      };
      changes::add(call.store(), name, &given, BasePromptGiven::Keep).await?;
    }

    accounts_answer(Judging::of(call.ground())).await
  }
}

crate::moved!(ClaudeLoginCode);

/// `DELETE /claude-logins/{login}`: a sign-in given up, its CLI ended and a new account's empty directory taken
/// away.
pub struct GiveUpClaudeLogin;

impl Route for GiveUpClaudeLogin {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/claude-logins/{login}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let login = call.param("login").unwrap_or_default().to_owned();
    let sign_in = SIGN_INS.lock().await.remove(&login).ok_or_else(|| no_sign_in(&login))?;
    if let Target::New { home, .. } = &sign_in.target {
      let _ = std::fs::remove_dir(home);
    }
    drop(sign_in);

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

crate::moved!(GiveUpClaudeLogin);
