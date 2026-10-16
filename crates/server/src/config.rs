//! The deployment's configuration: the names its agent and its owner go by (docs/policy.md, 「The
//! deployment's names」), where each service agency calls is reached (docs/api.md, 「Calls agency
//! makes」), the brain repository the editor line runs in (docs/launch-lines.md, 「What is fixed」), and the
//! trees its release is read from (docs/agent-control.md, 「The job's release」). Each is read through the
//! variables the deployment names it in.

use std::path::{Path, PathBuf};

use policy::deployment::{DEPLOYMENT_NAMES, Deployment};

use crate::peers::{PeerAddress, brain, cache_siren, cache_warmer, node};

/// The configuration a process runs under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
  pub names: Deployment,
  pub peers: PeerAddresses,
  /// The brain repository where the deployment names one; `brain_root` reads it with its default.
  pub brain: Option<PathBuf>,
  pub trees: Trees,
}

/// The variable naming the brain repository.
pub const BRAIN_ROOT: &str = "AGENCY_BRAIN_ROOT";

// The brain repository under the data root where the deployment names none.
const BRAIN_UNDER_DATA: &str = "brain";

/// The variables naming the trees the server's release is read from (docs/agent-control.md, 「The job's
/// release」).
pub const CODE_DIR: &str = "AGENCY_CODE_DIR";
pub const DEPLOY_DIR: &str = "AGENCY_DEPLOY_DIR";
pub const SOURCE_DIR: &str = "AGENCY_SOURCE_DIR";

// The deployment's data directory under the home, where the deploy tree's link stands unless named.
const INSTALLED_UNDER_HOME: &str = ".local/share/bmo";

/// The trees the deployment names, each none where its variable is unset or blank: the code tree it
/// started the server on, the deploy tree's link and the base checkout.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Trees {
  pub code: Option<PathBuf>,
  pub deploy: Option<PathBuf>,
  pub source: Option<PathBuf>,
}

impl Trees {
  fn read(variable: &impl Fn(&str) -> Option<String>) -> Self {
    let named = |name: &str| {
      variable(name)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    };

    Self {
      code: named(CODE_DIR),
      deploy: named(DEPLOY_DIR),
      source: named(SOURCE_DIR),
    }
  }

  /// The deploy tree's link: the named one, else `deploy` under the deployment's data directory in the
  /// home.
  pub fn deploy_link(&self, home: &Path) -> PathBuf {
    self.deploy.clone().unwrap_or_else(|| home.join(INSTALLED_UNDER_HOME).join("deploy"))
  }

  /// The base checkout: the named one, else the working directory.
  pub fn source_root(&self) -> PathBuf {
    self.source.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
  }
}

/// Where each service agency calls is reached, or none where the deployment names no address for it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeerAddresses {
  pub cache_siren: Option<PeerAddress>,
  pub cache_warmer: Option<PeerAddress>,
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
        cache_warmer: PeerAddress::read(&cache_warmer::ADDRESS, &variable)?,
        node: PeerAddress::read(&node::ADDRESS, &variable)?,
        brain: PeerAddress::read(&brain::ADDRESS, &variable)?,
      },
      brain: variable(BRAIN_ROOT)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from),
      trees: Trees::read(&variable),
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
