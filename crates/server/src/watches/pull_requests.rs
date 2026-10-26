//! Pull request reviews (docs/pull-request-reviews.md): GitHub's delivery read and checked against the webhook
//! secret, the fallback's one search an hour, a review item filed for the worker that holds the repository, and
//! a merge letting the items that waited on the pull request go on. No model runs here.

use std::collections::BTreeMap;
use std::time::Duration;

use hmac::{Hmac, Mac};
use policy::fill_marks;
use policy::pull_requests::PULL_REQUESTS;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use sha2::Sha256;
use store::all;

use crate::job_end::{address, token};

// How long a call of the job service or of GitHub waits.
const ANSWER_WITHIN: Duration = Duration::from_secs(20);

/// The variable naming the webhook secret, and the one naming the file it stands in.
pub const SECRET: &str = "BMO_GITHUB_WEBHOOK_SECRET";
pub const SECRET_FILE: &str = "BMO_GITHUB_WEBHOOK_SECRET_FILE";
/// The variable naming the organisation the fallback searches.
pub const ORGANISATION: &str = "BMO_GITHUB_ORG";

// The host a GitHub remote names.
const GITHUB: &str = "github.com";

/// A pull request as agency acts on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PullRequest {
  pub url: String,
  /// `owner/name`.
  pub repository: String,
  pub number: i64,
  pub title: String,
  pub head: Option<String>,
}

/// What a delivery or the fallback asks of agency.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Delivered {
  Open(PullRequest),
  Pushed(PullRequest),
  Merged(PullRequest),
  Closed(PullRequest),
}

/// Whether a delivery's `X-Hub-Signature-256` is the body's signature under the secret.
pub fn signed(secret: &str, body: &[u8], signature: &str) -> bool {
  let Some(given) = signature.strip_prefix("sha256=").and_then(|hex| hex::decode(hex).ok()) else {
    return false;
  };
  let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
    return false;
  };

  mac.update(body);
  mac.verify_slice(&given).is_ok()
}

/// What a `pull_request` delivery asks, or nothing for an event or an action agency does not act on.
pub fn read_delivery(event: &str, body: &Value) -> Option<Delivered> {
  if event != "pull_request" {
    return None;
  }

  let pull = &body["pull_request"];
  let request = PullRequest {
    url: pull["html_url"].as_str()?.to_owned(),
    repository: body["repository"]["full_name"].as_str()?.to_owned(),
    number: pull["number"].as_i64().or_else(|| body["number"].as_i64())?,
    title: pull["title"].as_str().unwrap_or_default().to_owned(),
    head: pull["head"]["sha"].as_str().map(str::to_owned),
  };

  match body["action"].as_str()? {
    "opened" | "reopened" | "ready_for_review" => Some(Delivered::Open(request)),
    "synchronize" => Some(Delivered::Pushed(request)),
    "closed" if pull["merged"].as_bool() == Some(true) => Some(Delivered::Merged(request)),
    "closed" => Some(Delivered::Closed(request)),
    _ => None,
  }
}

/// The `owner/name` a GitHub remote names, or nothing for a remote that is no GitHub repository.
pub fn repository_of(remote: &str) -> Option<String> {
  let remote = remote.trim();
  // The scheme where it names one, then a user before the host where it names one, then the host.
  let rest = ["ssh://", "https://", "http://"]
    .iter()
    .find_map(|scheme| remote.strip_prefix(scheme))
    .unwrap_or(remote);
  let rest = rest.split_once('@').map_or(rest, |(_, after)| after);
  let path = rest.strip_prefix(GITHUB).and_then(|after| after.strip_prefix([':', '/']))?;
  let path = path.trim_end_matches('/').trim_end_matches(".git");
  let mut parts = path.split('/');
  let (owner, name) = (parts.next()?, parts.next()?);

  (parts.next().is_none() && !owner.is_empty() && !name.is_empty()).then(|| format!("{owner}/{name}"))
}

/// The review item a pull request is filed as for the worker that holds its repository.
pub fn review_item(request: &PullRequest, worker: &str) -> Value {
  let marks = [
    ("repository", request.repository.as_str()),
    ("number", &request.number.to_string()),
    ("title", request.title.as_str()),
    ("url", request.url.as_str()),
  ];

  json!({
    "title": fill_marks(&PULL_REQUESTS.title, &marks),
    "brief": fill_marks(&PULL_REQUESTS.brief, &marks),
    "queueName": PULL_REQUESTS.queue,
    "assigneeName": worker,
    "requesterName": PULL_REQUESTS.requester,
    "labels": { "pr": request.url, "lane": PULL_REQUESTS.lane },
  })
}

/// The workers that hold a GitHub repository, by `owner/name` in lower case: each worker's checkout's
/// `origin` remote.
pub async fn holders(db: &DatabaseConnection) -> BTreeMap<String, String> {
  let rows = all(
    db,
    r#"SELECT "name", "cwd" FROM "agents" WHERE "kind" = 'worker' AND "cwd" IS NOT NULL AND "support_of" IS NULL
       ORDER BY "name""#,
    vec![],
  )
  .await
  .unwrap_or_default();
  let mut held = BTreeMap::new();

  for row in rows {
    let (Ok(name), Ok(cwd)) = (row.try_get::<String>("", "name"), row.try_get::<String>("", "cwd")) else {
      continue;
    };
    let remote = tokio::process::Command::new("git")
      .args(["-C", &cwd, "remote", "get-url", "origin"])
      .output()
      .await
      .ok()
      .filter(|out| out.status.success())
      .map(|out| String::from_utf8_lossy(&out.stdout).into_owned());

    if let Some(repository) = remote.as_deref().and_then(repository_of) {
      held.entry(repository.to_lowercase()).or_insert(name);
    }
  }

  held
}

// The job service as the watch reaches it: its address and the deployment's token.
struct Jobs {
  url: String,
  token: String,
  client: reqwest::Client,
}

impl Jobs {
  fn of(vars: &BTreeMap<String, String>) -> Option<Self> {
    Some(Self {
      url: address(vars, "BMO_JOB_URL")?,
      token: token(vars, "BMO_JOB_TOKEN", "BMO_JOB_TOKEN_FILE")?,
      client: reqwest::Client::new(),
    })
  }

  async fn send(
    &self,
    method: reqwest::Method,
    path: &str,
    query: &[(&str, &str)],
    body: Option<&Value>,
  ) -> Result<Value, String> {
    let mut asked = self
      .client
      .request(method, format!("{}{path}", self.url))
      .query(query)
      .bearer_auth(&self.token);

    if let Some(body) = body {
      asked = asked.json(body);
    }

    let answer = asked.timeout(ANSWER_WITHIN).send().await.map_err(|why| why.to_string())?;
    let status = answer.status();
    let read = answer.json::<Value>().await.unwrap_or(Value::Null);

    if status.is_success() {
      Ok(read)
    } else {
      Err(format!("the job service answered {} to {path}: {}", status.as_u16(), read["error"]))
    }
  }

  // The items labelled with a pull request's address that still stand.
  async fn standing(&self, url: &str) -> Result<Vec<Value>, String> {
    let label = format!("pr={url}");
    let page = self
      .send(reqwest::Method::GET, "/items", &[("label", &label), ("limit", "100")], None)
      .await?;

    Ok(
      page["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| !matches!(item["state"].as_str(), Some("done" | "dropped" | "merged")))
        .cloned()
        .collect(),
    )
  }

  async fn note(&self, item: &Value, text: &str) -> Result<(), String> {
    let id = item["id"].as_i64().unwrap_or_default();

    self
      .send(
        reqwest::Method::POST,
        &format!("/items/{id}/notes"),
        &[],
        Some(&json!({ "text": text, "actorName": PULL_REQUESTS.requester })),
      )
      .await
      .map(|_| ())
  }
}

fn is_review(item: &Value) -> bool {
  item["labels"]["lane"].as_str() == Some(PULL_REQUESTS.lane.as_str())
}

/// One delivery acted on: an open pull request a worker holds filed once for that worker, a push told on its
/// item, a merge opening the items that waited on it, a close told.
pub async fn act(
  vars: &BTreeMap<String, String>,
  held: &BTreeMap<String, String>,
  delivered: &Delivered,
) -> Result<(), String> {
  let Some(jobs) = Jobs::of(vars) else { return Ok(()) };

  match delivered {
    Delivered::Open(request) => {
      // A repository no worker holds is let be, and a pull request already filed is filed once.
      if let Some(worker) = held.get(&request.repository.to_lowercase())
        && !jobs.standing(&request.url).await?.iter().any(is_review)
      {
        jobs
          .send(reqwest::Method::POST, "/items", &[], Some(&review_item(request, worker)))
          .await?;
        println!("pull requests: {} filed for {worker}", request.url);
      }
    }
    Delivered::Pushed(request) => {
      let head = request.head.as_deref().unwrap_or_default();

      for item in jobs.standing(&request.url).await?.iter().filter(|item| is_review(item)) {
        jobs.note(item, &fill_marks(&PULL_REQUESTS.pushed, &[("head", head)])).await?;
      }
    }
    Delivered::Merged(request) => {
      let marks = [("url", request.url.as_str())];

      for item in jobs.standing(&request.url).await? {
        if is_review(&item) {
          jobs.note(&item, &fill_marks(&PULL_REQUESTS.merged_elsewhere, &marks)).await?;
        } else if item["state"].as_str() == Some("held") {
          let id = item["id"].as_i64().unwrap_or_default();

          jobs.note(&item, &fill_marks(&PULL_REQUESTS.merged, &marks)).await?;
          jobs
            .send(reqwest::Method::PATCH, &format!("/items/{id}"), &[], Some(&json!({ "state": "open" })))
            .await?;
        }
      }
    }
    Delivered::Closed(request) => {
      for item in jobs.standing(&request.url).await? {
        jobs
          .note(&item, &fill_marks(&PULL_REQUESTS.closed, &[("url", request.url.as_str())]))
          .await?;
      }
    }
  }

  Ok(())
}

/// The open pull requests one page of GitHub's search answers, as the fallback reads them.
pub fn searched(page: &Value) -> Vec<PullRequest> {
  page["items"]
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(|item| {
      let url = item["pull_request"]["html_url"].as_str().or_else(|| item["html_url"].as_str())?;
      let repository = item["repository_url"].as_str()?.rsplitn(3, '/').take(2).collect::<Vec<_>>();
      let [name, owner] = repository.as_slice() else { return None };

      Some(PullRequest {
        url: url.to_owned(),
        repository: format!("{owner}/{name}"),
        number: item["number"].as_i64()?,
        title: item["title"].as_str().unwrap_or_default().to_owned(),
        head: None,
      })
    })
    .collect()
}

/// The fallback: one search for every open pull request of the organisation, and each a delivery missed
/// filed; nothing where the deployment names no organisation.
pub async fn fallback(db: &DatabaseConnection, vars: &BTreeMap<String, String>) -> Result<(), String> {
  let Some(organisation) = vars.get(ORGANISATION).map(|org| org.trim()).filter(|org| !org.is_empty())
  else {
    return Ok(());
  };
  let query = format!("q=is:pr is:open org:{organisation}");
  let per_page = format!("per_page={}", PULL_REQUESTS.page);
  let out = tokio::process::Command::new("gh")
    .args(["api", "-X", "GET", "search/issues", "-f", &query, "-f", &per_page])
    .output()
    .await
    .map_err(|why| format!("gh: {why}"))?;

  if !out.status.success() {
    return Err(format!("gh: {}", String::from_utf8_lossy(&out.stderr).trim()));
  }

  let page: Value = serde_json::from_slice(&out.stdout).map_err(|why| format!("gh's answer: {why}"))?;
  let held = holders(db).await;

  for request in searched(&page) {
    act(vars, &held, &Delivered::Open(request)).await?;
  }

  Ok(())
}

/// The webhook secret the deployment names, or nothing.
pub fn secret(vars: &BTreeMap<String, String>) -> Option<String> {
  token(vars, SECRET, SECRET_FILE)
}
