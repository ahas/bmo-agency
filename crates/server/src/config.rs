//! The deployment's configuration: the names its agent and its owner go by (docs/policy.md, 「The
//! deployment's names」), where each service agency calls is reached (docs/api.md, 「Calls agency
//! makes」), and the brain repository the editor line runs in (docs/launch-lines.md, 「What is fixed」).
//! Each is read through the variables the deployment names it in.

use std::path::{Path, PathBuf};

use policy::deployment::{DEPLOYMENT_NAMES, Deployment};

use crate::peers::{PeerAddress, brain, cache_siren, node};

/// The configuration a process runs under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
  pub names: Deployment,
  pub peers: PeerAddresses,
  /// The brain repository where the deployment names one; `brain_root` reads it with its default.
  pub brain: Option<PathBuf>,
}

/// The variable naming the brain repository.
pub const BRAIN_ROOT: &str = "AGENCY_BRAIN_ROOT";

// The brain repository under the data root where the deployment names none.
const BRAIN_UNDER_DATA: &str = "brain";

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
      brain: variable(BRAIN_ROOT)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from),
    })
  }

  /// The brain repository: the folder the deployment names, else `brain` under the data root.
  pub fn brain_root(&self, data_root: &Path) -> PathBuf {
    self.brain.clone().unwrap_or_else(|| data_root.join(BRAIN_UNDER_DATA))
  }

  /// The configuration this process's environment names.
  pub fn of_process() -> Result<Self, String> {
    Self::read(|name| std::env::var(name).ok())
  }
}
