//! A renamed worker's checkout moved to the directory its rename names (docs/worker-slots.md, 「A worker
//! of its own in a worktree」), and its row pointed there only once it stands there. A job of the worker
//! at work holds the checkout's lock and puts the move off to the next pass.

use std::path::Path;

use policy::agent_renames::AGENT_RENAMES;
use store::run;

use super::{AgentRow, Ground, locks};

async fn git(cwd: &Path, args: &[&str]) -> Option<String> {
  let output = tokio::process::Command::new("git")
    .arg("-C")
    .arg(cwd)
    .args(args)
    .env("GIT_TERMINAL_PROMPT", "0")
    .stdin(std::process::Stdio::null())
    .output()
    .await
    .ok()?;

  output
    .status
    .success()
    .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// The checkout a linked worktree belongs to; none for a directory that is no linked worktree.
async fn main_checkout_of(cwd: &Path) -> Option<String> {
  let own = git(cwd, &["rev-parse", "--path-format=absolute", "--git-dir"]).await?;
  let common = git(cwd, &["rev-parse", "--path-format=absolute", "--git-common-dir"]).await?;

  if Path::new(&own) == Path::new(&common) {
    return None;
  }

  common.strip_suffix("/.git").map(str::to_owned)
}

/// The worktree moved under its checkout's lock, taken without waiting; whether it now stands there.
async fn move_worktree(ground: &Ground, from: &str, to: &str) -> bool {
  let Some(main) = main_checkout_of(Path::new(from)).await else { return false };

  if Path::new(to).exists() {
    return false;
  }

  let Some(_held) = locks::try_hold(&locks::lock_path(&ground.roots.data, from)) else {
    return false;
  };

  git(Path::new(&main), &["worktree", "move", from, to]).await.is_some() && Path::new(to).exists()
}

/// One worker's checkout moved from the directory its row still names to the one its rename names, and
/// the row pointed there once it stands there; whether it moved.
pub async fn move_checkout(ground: &Ground, worker: &str, from: &str, to: &str) -> bool {
  match AgentRow::named(&ground.db, worker).await {
    Ok(Some(row)) if row.node.is_none() && row.cwd.as_deref() == Some(from) => {}
    Ok(_) => return false,
    Err(refusal) => {
      eprintln!("{worker}: {}", refusal.detail);

      return false;
    }
  }

  if !move_worktree(ground, from, to).await {
    return false;
  }

  match run(
    &ground.db,
    r#"UPDATE "agents" SET "cwd" = $1 WHERE "name" = $2"#,
    vec![to.into(), worker.into()],
  )
  .await
  {
    Ok(_) => {
      println!("{worker}: checkout moved to {to}");

      true
    }
    Err(fault) => {
      eprintln!("{worker}: the moved checkout is not written: {fault}");

      false
    }
  }
}

/// Every renamed worker whose row still names its old directory, moved.
pub async fn move_renamed(ground: &Ground) {
  for rename in &AGENT_RENAMES.renames {
    if let Some(cwd) = &rename.cwd {
      move_checkout(ground, &rename.to, &cwd.from, &cwd.to).await;
    }
  }
}
