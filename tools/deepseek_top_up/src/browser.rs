//! The machine's own Chrome, started for one call and taken down with everything it started when the call ends
//! (docs/tools.md, 「How it pays」).
//!
//! DeepSeek's platform stands behind a bot check that refuses a headless Chrome and shows a human check to a
//! Chrome started with automation switches, so Chrome is started as a person's browser is: stable, not headless,
//! on a virtual screen of its own that `xvfb-run` raises and takes down, with no automation switch, and driven
//! over its loopback DevTools endpoint. Its cross-site frames run in the page's process, so a payment processor's
//! frames are reached through the page's own session. What is typed goes in through DevTools' text input and is
//! never written anywhere by this module.

use std::collections::HashMap;
use std::fs::{File, TryLockError};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::dom::{
  FocusParams, GetBoxModelParams, ScrollIntoViewIfNeededParams,
};
use chromiumoxide::cdp::browser_protocol::input::{
  DispatchMouseEventParams, DispatchMouseEventType, InsertTextParams, MouseButton,
};
use chromiumoxide::cdp::browser_protocol::page::{
  CreateIsolatedWorldParams, FrameId, FrameTree, GetFrameTreeParams, NavigateParams,
};
use chromiumoxide::cdp::js_protocol::runtime::{
  CallFunctionOnParams, EvaluateParams, ExecutionContextId, RemoteObject, RemoteObjectId,
};
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::task::JoinHandle;

/// Where the browser keeps its profile and how it is started.
#[derive(Debug, Clone)]
pub struct Place {
  /// The folder the profile, its lock and the browser's runtime folder stand in.
  pub folder: PathBuf,
  /// The browser's executable and the virtual screen it is started on.
  pub chrome: String,
  pub screen: String,
  /// Arguments the caller adds to the browser's own; none for the platform's payment.
  pub added: Vec<String>,
  /// How long the browser has to answer on its DevTools endpoint.
  pub start: Duration,
}

impl Place {
  /// The profile the browser keeps between calls.
  pub fn profile(&self) -> PathBuf {
    self.folder.join("profile")
  }
}

/// Why no browser is there to drive.
#[derive(Debug)]
pub enum Unopened {
  /// Another call holds the profile.
  Busy,
  /// The browser could not be started or reached, with why; the words carry no value typed into a page.
  Start(String),
}

/// A browser stopped answering a step; the step is named by the caller, never by the browser's own words.
#[derive(Debug)]
pub struct Lost;

impl From<chromiumoxide::error::CdpError> for Lost {
  fn from(_: chromiumoxide::error::CdpError) -> Self {
    Self
  }
}

/// What a find looks for in a frame: a field to type into, or a control to press, by the words it shows.
pub enum Wanted<'a> {
  /// An editable field, by its autocomplete purposes or the words of its placeholder, label, name or id;
  /// `password` takes the frame's password field first, and `alone` the frame's only editable field where no
  /// other matches.
  Field {
    words: &'a [String],
    purposes: &'a [&'a str],
    password: bool,
    alone: bool,
  },
  /// A visible control that is not disabled, whose own text is one of the words, the first word first.
  Control { words: &'a [String] },
}

impl Wanted<'_> {
  fn spec(&self) -> Value {
    match self {
      Self::Field {
        words,
        purposes,
        password,
        alone,
      } => {
        json!({ "kind": "field", "words": words, "purposes": purposes, "password": password, "alone": alone })
      }
      Self::Control { words } => json!({ "kind": "control", "words": words, "purposes": [] }),
    }
  }
}

/// The find run inside a frame's isolated world: the element `spec` asks for, or null.
const FIND: &str = r#"(spec) => {
  const lower = (text) => (text || "").toLowerCase().trim();
  const words = spec.words.map(lower);
  const shown = (element) => {
    const box = element.getBoundingClientRect();
    const style = getComputedStyle(element);
    return box.width > 0 && box.height > 0 && style.visibility !== "hidden" && style.display !== "none";
  };
  if (spec.kind === "field") {
    const skipped = ["hidden", "radio", "checkbox", "submit", "button", "file", "image", "reset"];
    const fields = [...document.querySelectorAll("input, textarea")].filter(
      (field) => shown(field) && !field.disabled && !field.readOnly && !skipped.includes(lower(field.type)),
    );
    if (spec.password) {
      const password = fields.find((field) => lower(field.type) === "password");
      if (password) return password;
    }
    const said = (field) =>
      [field.placeholder, field.getAttribute("aria-label"), field.name, field.id, ...[...(field.labels || [])].map((label) => label.innerText)]
        .map(lower)
        .join(" | ");
    const found = fields.find(
      (field) => spec.purposes.includes(lower(field.autocomplete)) || words.some((word) => said(field).includes(word)),
    );
    if (found) return found;
    return spec.alone && fields.length === 1 ? fields[0] : null;
  }
  const disabled = (element) => Boolean(element.closest('[disabled], [aria-disabled="true"], [class*="disabled"]'));
  const own = (element) => lower(element.innerText || element.value);
  const elements = [...document.querySelectorAll("body *")].filter(shown);
  for (const word of words) {
    const found = elements.find(
      (element) => own(element) === word && ![...element.children].some((child) => own(child) === word) && !disabled(element),
    );
    if (found) return found;
  }
  return null;
}"#;

/// One browser, its virtual screen and the one page a call drives, held under the profile's lock.
pub struct Browser {
  screen: Child,
  handler: JoinHandle<()>,
  browser: chromiumoxide::Browser,
  page: Page,
  worlds: HashMap<String, ExecutionContextId>,
  _lock: File,
}

/// The profile's preferences on its first start: no form, card or password of what is typed is kept.
fn first_preferences(profile: &Path) -> std::io::Result<()> {
  let defaults = profile.join("Default");
  let preferences = defaults.join("Preferences");

  if preferences.exists() {
    return Ok(());
  }

  std::fs::create_dir_all(&defaults)?;
  std::fs::write(
    preferences,
    json!({
      "autofill": { "enabled": false, "profile_enabled": false, "credit_card_enabled": false },
      "credentials_enable_service": false,
      "profile": { "password_manager_enabled": false },
    })
    .to_string(),
  )
}

/// The arguments the browser is started with, beside the executable: its profile, the password store kept in the
/// profile so no keyring of the person's session is asked, English pages, cross-site frames in the page's own
/// process, no crash reporter, and a loopback DevTools endpoint on a port it picks itself.
pub fn chrome_arguments(profile: &Path) -> Vec<String> {
  vec![
    format!("--user-data-dir={}", profile.display()),
    "--password-store=basic".to_owned(),
    "--no-first-run".to_owned(),
    "--no-default-browser-check".to_owned(),
    "--lang=en-US".to_owned(),
    "--disable-site-isolation-trials".to_owned(),
    "--disable-features=IsolateOrigins,site-per-process".to_owned(),
    "--disable-crash-reporter".to_owned(),
    "--disable-breakpad".to_owned(),
    "--ozone-platform=x11".to_owned(),
    "--window-size=1280,900".to_owned(),
    "--remote-debugging-address=127.0.0.1".to_owned(),
    "--remote-debugging-port=0".to_owned(),
    "about:blank".to_owned(),
  ]
}

/// The DevTools address the browser wrote into its profile once it listens: its port and its path.
fn devtools_address(profile: &Path) -> Option<String> {
  let written = std::fs::read_to_string(profile.join("DevToolsActivePort")).ok()?;
  let mut lines = written.lines();
  let port = lines.next()?.trim().parse::<u16>().ok()?;
  let path = lines.next()?.trim();

  path.starts_with('/').then(|| format!("ws://127.0.0.1:{port}{path}"))
}

/// Ends a process group: asked to end first, so `xvfb-run` clears its screen's files, then killed with whatever
/// is left, the screen's own process waited on.
fn end_group(child: &mut Child) {
  let Ok(group) = libc::pid_t::try_from(child.id()) else {
    return;
  };

  // SAFETY: `kill` reads only its two integer arguments; a negative pid names the group the child leads.
  unsafe { libc::kill(-group, libc::SIGTERM) };

  for _ in 0..30 {
    if matches!(child.try_wait(), Ok(Some(_))) {
      break;
    }

    std::thread::sleep(Duration::from_millis(100));
  }

  // SAFETY: as above.
  unsafe { libc::kill(-group, libc::SIGKILL) };
  let _ = child.wait();
}

impl Browser {
  /// The browser started on its profile under the profile's lock, its DevTools endpoint reached and a page opened.
  pub async fn open(place: &Place) -> Result<Self, Unopened> {
    let profile = place.profile();
    let run = place.folder.join("run");

    // The profile holds the platform's sign-in, so its folder is the account's alone.
    for folder in [&profile, &run] {
      std::fs::create_dir_all(folder).map_err(|why| Unopened::Start(format!("its folder: {why}")))?;
    }

    std::fs::set_permissions(&place.folder, std::fs::Permissions::from_mode(0o700))
      .map_err(|why| Unopened::Start(format!("its folder: {why}")))?;

    let lock = File::create(place.folder.join("browser.lock"))
      .map_err(|why| Unopened::Start(format!("its lock: {why}")))?;

    match lock.try_lock() {
      Ok(()) => {}
      Err(TryLockError::WouldBlock) => return Err(Unopened::Busy),
      Err(TryLockError::Error(why)) => return Err(Unopened::Start(format!("its lock: {why}"))),
    }

    first_preferences(&profile).map_err(|why| Unopened::Start(format!("its preferences: {why}")))?;
    let _ = std::fs::remove_file(profile.join("DevToolsActivePort"));

    let mut screen = Command::new("xvfb-run")
      .arg("--auto-servernum")
      .arg(format!("--server-args=-screen 0 {} -nolisten tcp", place.screen))
      .arg(&place.chrome)
      .args(chrome_arguments(&profile))
      .args(&place.added)
      .env_clear()
      .env("PATH", std::env::var_os("PATH").unwrap_or_default())
      .env(
        "HOME",
        std::env::var_os("HOME").unwrap_or_else(|| place.folder.clone().into_os_string()),
      )
      .env("XDG_RUNTIME_DIR", &run)
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .process_group(0)
      .spawn()
      .map_err(|why| Unopened::Start(format!("xvfb-run could not run: {why}")))?;

    let address = match Self::listening(&mut screen, &profile, place.start).await {
      Ok(address) => address,
      Err(why) => {
        end_group(&mut screen);
        return Err(Unopened::Start(why));
      }
    };

    let (browser, mut handler) = match chromiumoxide::Browser::connect(address).await {
      Ok(connected) => connected,
      Err(why) => {
        end_group(&mut screen);
        return Err(Unopened::Start(format!("its DevTools endpoint did not answer: {why}")));
      }
    };
    let handler = tokio::spawn(async move { while handler.next().await.is_some() {} });

    // The page is brought to the window's front, as a person's tab is: a tab behind another gets no input.
    let opened = match browser.new_page("about:blank").await {
      Ok(page) => match page.bring_to_front().await {
        Ok(_) => Ok(page),
        Err(why) => Err(why),
      },
      Err(why) => Err(why),
    };

    let opened = match opened {
      Ok(page) => Self::sized(page, place.start).await,
      Err(why) => Err(format!("no page could be opened: {why}")),
    };

    match opened {
      Ok(page) => Ok(Self {
        screen,
        handler,
        browser,
        page,
        worlds: HashMap::new(),
        _lock: lock,
      }),
      Err(why) => {
        handler.abort();
        end_group(&mut screen);
        Err(Unopened::Start(why))
      }
    }
  }

  /// The page once its window has a size on the screen, within the start wait: a window still being mapped has
  /// none, and a press there would land nowhere.
  async fn sized(page: Page, start: Duration) -> Result<Page, String> {
    let deadline = tokio::time::Instant::now() + start;

    loop {
      let sized = page.evaluate_expression("innerWidth > 0 && innerHeight > 0").await;

      if sized.ok().and_then(|answered| answered.into_value::<bool>().ok()) == Some(true) {
        return Ok(page);
      }

      if tokio::time::Instant::now() >= deadline {
        return Err(format!("its window had no size within {} seconds", start.as_secs()));
      }

      tokio::time::sleep(Duration::from_millis(100)).await;
    }
  }

  /// The DevTools address, once the browser has written it, within the start wait.
  async fn listening(screen: &mut Child, profile: &Path, start: Duration) -> Result<String, String> {
    let deadline = tokio::time::Instant::now() + start;

    loop {
      if let Some(address) = devtools_address(profile) {
        return Ok(address);
      }

      if let Ok(Some(status)) = screen.try_wait() {
        return Err(format!("it ended before it listened, {status}"));
      }

      if tokio::time::Instant::now() >= deadline {
        return Err(format!("it did not listen within {} seconds", start.as_secs()));
      }

      tokio::time::sleep(Duration::from_millis(100)).await;
    }
  }

  /// Sends the page to an address; what it shows is waited for by the caller.
  pub async fn go(&mut self, address: &str) -> Result<(), Lost> {
    self.page.execute(NavigateParams::new(address)).await?;
    self.worlds.clear();
    Ok(())
  }

  /// The page's frames, the main frame first, each with its address.
  pub async fn frames(&self) -> Result<Vec<(FrameId, String)>, Lost> {
    fn walk(tree: &FrameTree, into: &mut Vec<(FrameId, String)>) {
      into.push((tree.frame.id.clone(), tree.frame.url.clone()));
      for child in tree.child_frames.iter().flatten() {
        walk(child, into);
      }
    }

    let tree = self.page.execute(GetFrameTreeParams::default()).await?.result.frame_tree;
    let mut frames = Vec::new();

    walk(&tree, &mut frames);
    Ok(frames)
  }

  /// A world of the tool's own in a frame, kept until the frame's document changes.
  async fn world(&mut self, frame: &FrameId) -> Result<ExecutionContextId, Lost> {
    if let Some(world) = self.worlds.get(frame.inner()) {
      return Ok(*world);
    }

    let made = self
      .page
      .execute(
        CreateIsolatedWorldParams::builder()
          .frame_id(frame.clone())
          .world_name("top-up")
          .build()
          .map_err(|_| Lost)?,
      )
      .await?
      .result
      .execution_context_id;

    self.worlds.insert(frame.inner().clone(), made);
    Ok(made)
  }

  /// An expression evaluated in a frame's world, made again once where the frame's document has changed.
  async fn evaluate(
    &mut self,
    frame: &FrameId,
    expression: &str,
    by_value: bool,
  ) -> Result<RemoteObject, Lost> {
    for _ in 0..2 {
      let world = self.world(frame).await?;
      let asked = EvaluateParams::builder()
        .expression(expression)
        .context_id(world)
        .return_by_value(by_value)
        .build()
        .map_err(|_| Lost)?;

      match self.page.execute(asked).await {
        Ok(answered) if answered.result.exception_details.is_none() => return Ok(answered.result.result),
        _ => {
          self.worlds.remove(frame.inner());
        }
      }
    }

    Err(Lost)
  }

  /// The text a frame shows, lowered.
  pub async fn text(&mut self, frame: &FrameId) -> Result<String, Lost> {
    let shown = self.evaluate(frame, "document.body ? document.body.innerText : ''", true).await?;

    Ok(shown.value.and_then(|value| value.as_str().map(str::to_lowercase)).unwrap_or_default())
  }

  /// The element a frame holds for what is wanted, or none.
  pub async fn find(
    &mut self,
    frame: &FrameId,
    wanted: &Wanted<'_>,
  ) -> Result<Option<RemoteObjectId>, Lost> {
    let found = self.evaluate(frame, &format!("({FIND})({})", wanted.spec()), false).await?;

    Ok(found.object_id)
  }

  /// The first frame holding what is wanted, and the element there.
  pub async fn find_anywhere(&mut self, wanted: &Wanted<'_>) -> Result<Option<RemoteObjectId>, Lost> {
    for (frame, _) in self.frames().await? {
      if let Ok(Some(found)) = self.find(&frame, wanted).await {
        return Ok(Some(found));
      }
    }

    Ok(None)
  }

  /// A press of an element where a person's pointer would press it: its middle, once it is scrolled into view.
  pub async fn press(&self, element: &RemoteObjectId) -> Result<(), Lost> {
    self
      .page
      .execute(ScrollIntoViewIfNeededParams::builder().object_id(element.clone()).build())
      .await?;

    let model = self
      .page
      .execute(GetBoxModelParams::builder().object_id(element.clone()).build())
      .await?;
    let corners = model.result.model.content.inner();
    let (x, y) = corners.chunks(2).fold((0.0, 0.0), |(x, y), corner| {
      (x + corner[0] / 4.0, y + corner.get(1).copied().unwrap_or(0.0) / 4.0)
    });

    for kind in [
      DispatchMouseEventType::MouseMoved,
      DispatchMouseEventType::MousePressed,
      DispatchMouseEventType::MouseReleased,
    ] {
      let pressing = !matches!(kind, DispatchMouseEventType::MouseMoved);
      let mut event = DispatchMouseEventParams::builder().r#type(kind).x(x).y(y);

      if pressing {
        event = event.button(MouseButton::Left).click_count(1);
      }

      self.page.execute(event.build().map_err(|_| Lost)?).await?;
    }

    Ok(())
  }

  /// Text typed into a field as a person's keyboard would, over whatever the field held. The text is handed to
  /// DevTools' text input alone.
  pub async fn type_into(&self, field: &RemoteObjectId, text: &str) -> Result<(), Lost> {
    let select = CallFunctionOnParams::builder()
      .function_declaration("function () { this.focus(); if (this.select) this.select(); }")
      .object_id(field.clone())
      .build()
      .map_err(|_| Lost)?;

    self.page.execute(select).await?;
    self.page.execute(FocusParams::builder().object_id(field.clone()).build()).await?;
    self.page.execute(InsertTextParams::new(text)).await?;
    Ok(())
  }
}

impl Browser {
  /// The browser closed through DevTools, as a person's window is closed, so what its profile holds is written
  /// out, and its screen left to end with it; whatever has not ended within ten seconds is ended by the drop.
  pub async fn close(&mut self) {
    let _ = tokio::time::timeout(Duration::from_secs(10), self.browser.close()).await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);

    while tokio::time::Instant::now() < deadline && matches!(self.screen.try_wait(), Ok(None)) {
      tokio::time::sleep(Duration::from_millis(100)).await;
    }
  }
}

impl Drop for Browser {
  fn drop(&mut self) {
    self.handler.abort();
    end_group(&mut self.screen);
  }
}
