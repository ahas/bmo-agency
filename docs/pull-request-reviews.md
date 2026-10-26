# Pull request reviews

How a pull request opened on one of the deployment's repositories reaches the worker that holds that
repository, with no person reading GitHub and no model watching it: GitHub tells agency, agency files a review
item for the worker, and a merge lets the item that waited on the pull request go on.

## What is watched

- **The repositories the workers hold.** A worker whose checkout's `origin` remote is a GitHub repository holds
  that repository's pull requests. Agency reads each worker's remote from its checkout as a delivery or the
  fallback needs it; a repository no worker holds is let be.
- **No model watches.** The watch is GitHub's delivery and one search an hour; a model runs only inside the
  review job of a pull request that exists.

## The delivery

- **GitHub pushes.** Each repository a worker holds sends its `pull_request` events
  (opened, reopened, synchronize, closed) to the public address, where the console's gateway hands each
  delivery, its body as it came and its headers, to agency's `POST /github/deliveries` (`docs/api.md`,
  「pull request reviews」).
- **It is GitHub's or nothing.** Agency checks the delivery's `X-Hub-Signature-256` against the webhook secret
  the deployment names (`BMO_GITHUB_WEBHOOK_SECRET`, or the file `BMO_GITHUB_WEBHOOK_SECRET_FILE` names); a
  delivery that does not carry it, or carries another, is refused 401 `bad-signature` and nothing is read of
  it. A deployment that names no secret takes no delivery: every one is refused 503 `no-webhook-secret`.
- **It is answered at once.** A delivery that reads is answered 202 and acted on after the answer; an event
  agency does not act on is answered 202 as well, so GitHub never retries it.

## The fallback

- **Once an hour, one search.** Agency asks GitHub's search for every open pull request of the repositories the
  owner's login the deployment names (`BMO_GITHUB_ORG`) owns, in one request (`gh api search/issues`, `is:pr
  is:open user:<login>`, a hundred to a page), and files what a delivery missed. A deployment that names no
  login runs no fallback.
- **The budget:** one GitHub call an hour for the fallback, a page more only past a hundred open pull requests,
  and none for a delivery; no model tokens at all for the watch.

## The review item

- **One item per pull request.** An open pull request a worker holds is filed in the job service as an item for
  that worker (`POST /items`): its title names the repository and the pull request's number and title, and its
  brief is the review the worker runs — hold the change to the base-library rule where the repository is a
  base library (the general case only, the base's own tests, nothing of one asker's), run the repository's
  checks on the pull request's branch, use bmo-review where it helps, then merge it or send it back with the
  reason as a review comment on the pull request, and tell the owner's room what became of it in one line.
  The words stand in `policy/pull-requests.toml`.
- **Its labels:** `pr`, the pull request's address, by which a second delivery or the fallback finds it filed
  and files nothing; and `lane`, `pr-review`, the job service's unattended lane, which opens an item of that
  lane without an approval and says so in the item's note where the deployment's job service holds it. Until
  then the item is a draft, approved as any other.
- **A pull request pushed again** (`synchronize`) whose item stands is told on it as a note naming the new
  head, and nothing more is filed.

## After the pull request

- **Merged:** every other item labelled with the same `pr` address — the item whose work stopped on the pull
  request — that stands `held` is opened again (`state` `open`) with a note saying the pull request merged, so
  its work goes on; and a review item still standing, its pull request merged by somebody else, is told so in a
  note.
- **Closed unmerged:** the review item still standing is told so in a note, and the items waiting on it are
  told as well; none is opened.
