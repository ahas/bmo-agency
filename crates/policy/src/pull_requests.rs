//! A pull request's review item and what is told on the items around it (docs/pull-request-reviews.md).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PullRequests {
  /// How often the fallback search runs, in seconds.
  pub fallback_seconds: u64,
  /// How many pull requests one page of the search holds.
  pub page: u32,
  /// The job service's queue the item stands in.
  pub queue: String,
  /// The lane label that opens the item without an approval where the job service holds that lane.
  pub lane: String,
  /// Who files the item.
  pub requester: String,
  pub title: String,
  pub brief: String,
  pub pushed: String,
  pub merged: String,
  pub merged_elsewhere: String,
  pub closed: String,
}

crate::part!(pub PULL_REQUESTS: PullRequests = "pull-requests.toml");
