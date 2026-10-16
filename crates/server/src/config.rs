//! The deployment's configuration: the names its agent and its owner go by (docs/policy.md, 「The
//! deployment's names」), and where each service agency calls is reached (docs/api.md, 「Calls agency
//! makes」). Each is read through the variables the deployment names it in.

use policy::deployment::{DEPLOYMENT_NAMES, Deployment};

use crate::peers::{PeerAddress, brain, cache_siren, node};

/// The configuration a process runs under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
  pub names: Deployment,
  pub peers: PeerAddresses,
}

/// Where each service agency calls is reached, or none where the deployment names no address for it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeerAddresses {
  pub cache_siren: Option<PeerAddress>,
  pub node: Option<PeerAddress>,
  pub brain: Option<PeerAddress>,
}

impl Config {
  /// The configuration read through `variable`; an address that does not read as a URL is a fault
  /// naming its variable.
  pub fn read(variable: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
    Ok(Self {
      names: DEPLOYMENT_NAMES.read(&variable),
      peers: PeerAddresses {
        cache_siren: PeerAddress::read(&cache_siren::ADDRESS, &variable)?,
        node: PeerAddress::read(&node::ADDRESS, &variable)?,
        brain: PeerAddress::read(&brain::ADDRESS, &variable)?,
      },
    })
  }

  /// The configuration this process's environment names.
  pub fn of_process() -> Result<Self, String> {
    Self::read(|name| std::env::var(name).ok())
  }
}
