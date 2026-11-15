//! One payment on DeepSeek's web platform (docs/tools.md, 「How it pays」 and 「Where a person is asked」): the
//! top-up page opened, a sign-in made with the login where the platform asks for one, a custom amount and the card
//! chosen, the card's fields typed into whichever frames the payment processor shows them in, pay pressed once,
//! and the page watched for the outcome. Nothing here presses pay a second time, and nothing goes on past a step
//! only a person can take.

use std::time::Duration;

use chromiumoxide::cdp::js_protocol::runtime::RemoteObjectId;
use policy::providers::deepseek::PageWords;
use tokio::time::Instant;
use url::Url;

use crate::browser::{Browser, Lost, Place, Unopened, Wanted};
use crate::secret::Saved;

/// The card's field keys, as the console's kind `card` writes them.
const NUMBER: &str = "CARD_NUMBER";
const EXPIRY: &str = "CARD_EXPIRY";
const CVC: &str = "CARD_CVC";
const HOLDER: &str = "CARD_HOLDER_NAME";
/// The login's field keys, as the console's kind `account-login` writes them.
const USERNAME: &str = "USERNAME";
const PASSWORD: &str = "PASSWORD";

/// The platform a call pays on and how long each of its waits lasts.
#[derive(Debug, Clone)]
pub struct Platform {
  /// The platform's origin and its two pages' paths.
  pub address: String,
  pub top_up_path: String,
  pub sign_in_path: String,
  pub place: Place,
  pub waits: Waits,
}

#[derive(Debug, Clone, Copy)]
pub struct Waits {
  /// For each step's page or control.
  pub step: Duration,
  /// For the outcome of the payment pressed, a person's confirmation included.
  pub confirm: Duration,
  /// How often a waited page is looked at.
  pub look: Duration,
}

impl Platform {
  /// The command a person runs at the machine's desktop to sign the tool's browser in: the same profile, opened
  /// in a window on the sign-in page. The profile is written from `$HOME` where it stands under it.
  pub fn sign_in_command(&self) -> String {
    let profile = self.place.profile();
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let shown = match home.as_deref().and_then(|home| profile.strip_prefix(home).ok()) {
      Some(under) => format!("$HOME/{}", under.display()),
      None => profile.display().to_string(),
    };

    format!(
      "{} --user-data-dir=\"{shown}\" --password-store=basic --lang=en-US {}{}",
      self.place.chrome, self.address, self.sign_in_path
    )
  }
}

/// Why a call stopped before pay was pressed: nothing was paid.
#[derive(Debug)]
pub enum Stop {
  Busy,
  /// The browser could not be started, with why.
  Browser(String),
  /// The browser stopped answering at a step.
  Lost(&'static str),
  /// A step's page or control did not show within the step wait.
  Unread(&'static str),
  /// The platform asks a person to finish the sign-in.
  PersonAsked,
  SignInRefused,
  IdentityAsked,
}

/// What came of pay, pressed once.
#[derive(Debug, PartialEq, Eq)]
pub enum Paid {
  /// The platform showed the payment made, pressed at this moment in epoch seconds.
  Paid {
    at: i64,
  },
  Refused,
  /// No outcome within the confirmation wait: a person's confirmation not given, or a page the tool could not read.
  Unconfirmed,
}

/// What the page shows at one look: the main frame's address and text, and every frame's address and text.
struct Look {
  address: String,
  text: String,
  frames: Vec<(String, String)>,
}

fn shows(text: &str, words: &[String]) -> bool {
  words.iter().any(|word| text.contains(&word.to_lowercase()))
}

/// The path of a web page's address; none for a page with no web address, as a blank page has.
fn web_path(address: &str) -> Option<String> {
  Url::parse(address)
    .ok()
    .filter(|address| matches!(address.scheme(), "http" | "https"))
    .map(|address| address.path().to_owned())
}

/// Whether a frame's address stands on one of the hosts, or under one.
fn on_host(address: &str, hosts: &[String]) -> bool {
  let host = Url::parse(address).ok().and_then(|address| address.host_str().map(str::to_owned));

  host.is_some_and(|host| hosts.iter().any(|named| host == *named || host.ends_with(&format!(".{named}"))))
}

struct Drive<'a> {
  browser: Browser,
  platform: &'a Platform,
  words: &'a PageWords,
}

impl<'a> Drive<'a> {
  async fn look(&mut self) -> Result<Look, Lost> {
    let frames = self.browser.frames().await?;
    let mut read = Vec::new();

    for (frame, address) in &frames {
      let text = self.browser.text(frame).await.unwrap_or_default();

      read.push((address.clone(), text));
    }

    let (address, text) = read.first().cloned().unwrap_or_default();

    Ok(Look {
      address,
      text,
      frames: read,
    })
  }

  fn person_asked(&self, look: &Look) -> bool {
    shows(&look.text, &self.words.person_asked)
      || look
        .frames
        .iter()
        .skip(1)
        .any(|(address, _)| on_host(address, &self.words.person_asked_frames))
  }

  async fn pause(&self) {
    tokio::time::sleep(self.platform.waits.look).await;
  }

  /// The main frame's control or field for what is wanted, waited for within the step wait.
  async fn wait_main(&mut self, wanted: &Wanted<'_>, what: &'static str) -> Result<RemoteObjectId, Stop> {
    let deadline = Instant::now() + self.platform.waits.step;

    loop {
      let frames = self.browser.frames().await.map_err(|_| Stop::Lost(what))?;

      if let Some((main, _)) = frames.first()
        && let Ok(Some(found)) = self.browser.find(main, wanted).await
      {
        return Ok(found);
      }

      if Instant::now() >= deadline {
        return Err(Stop::Unread(what));
      }

      self.pause().await;
    }
  }

  /// A field in whichever frame holds it, waited for within the step wait where `needed`, looked for once
  /// otherwise.
  async fn wait_any(
    &mut self,
    wanted: &Wanted<'_>,
    what: &'static str,
    needed: bool,
  ) -> Result<Option<RemoteObjectId>, Stop> {
    let deadline = Instant::now() + self.platform.waits.step;

    loop {
      if let Some(found) = self.browser.find_anywhere(wanted).await.map_err(|_| Stop::Lost(what))? {
        return Ok(Some(found));
      }

      if !needed {
        return Ok(None);
      }

      if Instant::now() >= deadline {
        return Err(Stop::Unread(what));
      }

      self.pause().await;
    }
  }

  async fn press_main(&mut self, words: &[String], what: &'static str) -> Result<(), Stop> {
    let control = self.wait_main(&Wanted::Control { words }, what).await?;

    self.browser.press(&control).await.map_err(|_| Stop::Lost(what))
  }

  async fn type_into(
    &mut self,
    field: &RemoteObjectId,
    text: &str,
    what: &'static str,
  ) -> Result<(), Stop> {
    self.browser.type_into(field, text).await.map_err(|_| Stop::Lost(what))
  }

  /// The top-up page opened and its form shown, signed in first with the login where the platform asks for it.
  async fn reach_form(&mut self, login: &Saved) -> Result<(), Stop> {
    let top_up = format!("{}{}", self.platform.address, self.platform.top_up_path);
    let custom = Wanted::Control {
      words: &self.words.custom_amount,
    };
    let mut signed = false;
    let mut sent_back = false;
    let mut deadline = Instant::now() + self.platform.waits.step;

    self.browser.go(&top_up).await.map_err(|_| Stop::Lost("the top-up page"))?;

    loop {
      let look = self.look().await.map_err(|_| Stop::Lost("the top-up page"))?;
      let Some(path) = web_path(&look.address) else {
        if Instant::now() >= deadline {
          return Err(Stop::Unread("the top-up page"));
        }

        self.pause().await;
        continue;
      };

      if self.person_asked(&look) {
        return Err(Stop::PersonAsked);
      }

      if shows(&look.text, &self.words.identity_asked) {
        return Err(Stop::IdentityAsked);
      }

      if path.starts_with(&self.platform.sign_in_path) {
        if signed {
          if shows(&look.text, &self.words.sign_in_refused) {
            return Err(Stop::SignInRefused);
          }
        } else {
          self.sign_in(login).await?;
          signed = true;
          deadline = Instant::now() + self.platform.waits.step;
        }
      } else if path.starts_with(&self.platform.top_up_path) {
        let frames = self.browser.frames().await.map_err(|_| Stop::Lost("the top-up page"))?;

        if let Some((main, _)) = frames.first()
          && let Ok(Some(_)) = self.browser.find(main, &custom).await
        {
          return Ok(());
        }
      } else if signed && !sent_back {
        // The platform's own landing page after a sign-in: the top-up page is asked for again, once.
        sent_back = true;
        self.browser.go(&top_up).await.map_err(|_| Stop::Lost("the top-up page"))?;
      }

      if Instant::now() >= deadline {
        return Err(if signed {
          Stop::Unread("the platform past the sign-in")
        } else {
          Stop::Unread("the top-up page")
        });
      }

      self.pause().await;
    }
  }

  /// The sign-in form filled with the login and sent, once.
  async fn sign_in(&mut self, login: &Saved) -> Result<(), Stop> {
    let what = "the sign-in form";
    let password_field = Wanted::Field {
      words: &self.words.password,
      purposes: &["current-password"],
      password: true,
      alone: false,
    };
    let frames = self.browser.frames().await.map_err(|_| Stop::Lost(what))?;
    let main = frames.first().map(|(frame, _)| frame.clone()).ok_or(Stop::Unread(what))?;

    if self.browser.find(&main, &password_field).await.ok().flatten().is_none()
      && let Ok(Some(tab)) = self
        .browser
        .find(
          &main,
          &Wanted::Control {
            words: &self.words.password_tab,
          },
        )
        .await
    {
      self.browser.press(&tab).await.map_err(|_| Stop::Lost(what))?;
    }

    let identity = self
      .wait_main(
        &Wanted::Field {
          words: &self.words.identity,
          purposes: &["username", "email", "tel"],
          password: false,
          alone: false,
        },
        what,
      )
      .await?;

    self.type_into(&identity, login.value(USERNAME), what).await?;

    let password = self.wait_main(&password_field, what).await?;

    self.type_into(&password, login.value(PASSWORD), what).await?;

    let words = self.words;

    self.press_main(&words.sign_in, what).await
  }

  /// The custom amount typed, the card chosen and its fields typed, up to the pay button, unpressed.
  async fn fill(&mut self, amount: &str, card: &Saved) -> Result<RemoteObjectId, Stop> {
    let words = self.words;

    self.press_main(&words.custom_amount, "the custom amount").await?;

    let amount_field = self
      .wait_main(
        &Wanted::Field {
          words: &words.amount,
          purposes: &[],
          password: false,
          alone: true,
        },
        "the amount's field",
      )
      .await?;

    self.type_into(&amount_field, amount, "the amount's field").await?;
    self.press_main(&words.card_method, "the card among the ways to pay").await?;

    let number = Wanted::Field {
      words: &words.card_number,
      purposes: &["cc-number"],
      password: false,
      alone: false,
    };
    let next = Wanted::Control {
      words: &words.next_step,
    };
    let deadline = Instant::now() + self.platform.waits.step;
    let mut went_on = false;

    let number_field = loop {
      if let Some(found) = self.wait_any(&number, "the card's fields", false).await? {
        break found;
      }

      if !went_on {
        let frames = self.browser.frames().await.map_err(|_| Stop::Lost("the next step"))?;

        if let Some((main, _)) = frames.first()
          && let Ok(Some(control)) = self.browser.find(main, &next).await
        {
          self.browser.press(&control).await.map_err(|_| Stop::Lost("the next step"))?;
          went_on = true;
        }
      }

      if Instant::now() >= deadline {
        return Err(Stop::Unread("the card's fields"));
      }

      self.pause().await;
    };

    self.type_into(&number_field, card.value(NUMBER), "the card's number field").await?;

    let expiry = self
      .wait_any(
        &Wanted::Field {
          words: &words.card_expiry,
          purposes: &["cc-exp"],
          password: false,
          alone: false,
        },
        "the card's expiry field",
        true,
      )
      .await?
      .ok_or(Stop::Unread("the card's expiry field"))?;

    self.type_into(&expiry, card.value(EXPIRY), "the card's expiry field").await?;

    let cvc = self
      .wait_any(
        &Wanted::Field {
          words: &words.card_cvc,
          purposes: &["cc-csc"],
          password: false,
          alone: false,
        },
        "the card's security code field",
        true,
      )
      .await?
      .ok_or(Stop::Unread("the card's security code field"))?;

    self.type_into(&cvc, card.value(CVC), "the card's security code field").await?;

    if let Some(holder) = self
      .wait_any(
        &Wanted::Field {
          words: &words.card_holder,
          purposes: &["cc-name"],
          password: false,
          alone: false,
        },
        "the card holder's field",
        false,
      )
      .await?
    {
      self.type_into(&holder, card.value(HOLDER), "the card holder's field").await?;
    }

    self.wait_main(&Wanted::Control { words: &words.pay }, "the pay button").await
  }

  /// The page watched after pay, pressed at `at` in epoch seconds, within the confirmation wait, for a payment made
  /// or refused. Nothing is pressed.
  async fn outcome(&mut self, at: i64) -> Paid {
    let deadline = Instant::now() + self.platform.waits.confirm;

    loop {
      if let Ok(look) = self.look().await {
        let shown = |words: &[String]| look.frames.iter().any(|(_, text)| shows(text, words));

        if shown(&self.words.paid) {
          return Paid::Paid { at };
        }

        if shown(&self.words.refused) {
          return Paid::Refused;
        }
      }

      if Instant::now() >= deadline {
        return Paid::Unconfirmed;
      }

      self.pause().await;
    }
  }
}

/// One payment of `amount`, typed as given, with the card, signed in with the login where the platform asks.
/// Pay is pressed once at most; a stop before it pays nothing. The browser is closed through DevTools whatever the
/// end, so its profile keeps what the platform stored for the next call.
pub async fn pay(
  platform: &Platform,
  words: &PageWords,
  login: &Saved,
  card: &Saved,
  amount: &str,
) -> Result<Paid, Stop> {
  let browser = Browser::open(&platform.place).await.map_err(|unopened| match unopened {
    Unopened::Busy => Stop::Busy,
    Unopened::Start(why) => Stop::Browser(why),
  })?;
  let mut drive = Drive {
    browser,
    platform,
    words,
  };
  let ended = drive.run(login, card, amount).await;

  drive.browser.close().await;
  ended
}

impl Drive<'_> {
  async fn run(&mut self, login: &Saved, card: &Saved, amount: &str) -> Result<Paid, Stop> {
    self.reach_form(login).await?;

    let pay = self.fill(amount, card).await?;
    let at = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map_or(0, |since| i64::try_from(since.as_secs()).unwrap_or(i64::MAX));

    if self.browser.press(&pay).await.is_err() {
      // The press may have reached the page before the browser stopped answering, so it is not called unpaid.
      return Ok(Paid::Unconfirmed);
    }

    Ok(self.outcome(at).await)
  }
}
