//! What agency reads off the account service's delegation chain for an organisation beside the prompt layers and
//! the supply sources: the agents an account is given as it is made ready (docs/role-agents.md, 「The agents an
//! account is given」), and who may add and change repositories and which a unit owns (docs/repositories.md,
//! 「Who may add and change a repository」 and 「An organisation's repository」); and a person's own changes over the
//! agents their organisation supplies (docs/role-agents.md, 「A person's own agents」).

pub mod composition;
pub mod own_agents;
pub mod repositories;
pub mod resources;

use axum::http::StatusCode;

use crate::control::Refused;

/// The refusal of a check the account service did not answer: nothing is guessed.
pub fn unchecked() -> Refused {
  Refused::new(
    StatusCode::SERVICE_UNAVAILABLE,
    &policy::control_words::CONTROL_WORDS.organisation_supply.unchecked,
  )
  .with_code("access-unchecked")
}

/// Whether the caller heads the unit, as the table of the units each account reaches says (`crate::unit_reach`);
/// the platform's call always does. The kind of resource a head check stands for names no other reach: heading a
/// unit is the whole of who changes its resources.
pub async fn heads(
  ground: &crate::places::Ground,
  acting: crate::access::Acting,
  _key: &str,
  unit: i64,
) -> Result<bool, Refused> {
  let crate::access::Acting::Person(person) = acting else { return Ok(true) };

  crate::unit_reach::heads(&ground.db, person, unit).await.map_err(|_| unchecked())
}
