//! The deployment's configuration: its names, the addresses of the services agency calls and the
//! brain repository, read through the variables that name them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use agency_server::config::{Config, PeerAddresses};
use policy::deployment::DEPLOYMENT_NAMES;

fn read(set: &[(&str, &str)]) -> Result<Config, String> {
  let variables: HashMap<String, String> =
    set.iter().map(|(name, value)| ((*name).to_owned(), (*value).to_owned())).collect();

  Config::read(|name| variables.get(name).cloned())
}

#[test]
fn nothing_named_calls_no_service_and_keeps_the_neutral_names() {
  let config = read(&[]).unwrap();

  assert_eq!(config.peers, PeerAddresses::default());
  assert_eq!(config.names, DEPLOYMENT_NAMES.read(|_| None));
}

#[test]
fn a_named_address_is_read_with_its_token() {
  let config = read(&[
    ("AGENCY_BRAIN_URL", " http://127.0.0.1:7000/brain "),
    ("AGENCY_BRAIN_TOKEN", "brain-token"),
    ("AGENCY_NODE_URL", "https://nodes.example/"),
    ("AGENCY_CACHE_SIREN_URL", "   "),
  ])
  .unwrap();
  let brain = config.peers.brain.unwrap();
  let node = config.peers.node.unwrap();

  assert_eq!(brain.url.as_str(), "http://127.0.0.1:7000/brain");
  assert_eq!(brain.token.as_deref(), Some("brain-token"));
  assert_eq!(node.url.as_str(), "https://nodes.example/");
  assert_eq!(node.token, None);
  assert_eq!(config.peers.cache_siren, None, "a blank address names no service");
}

#[test]
fn an_address_that_is_not_a_url_is_a_fault_naming_its_variable() {
  let fault = read(&[("AGENCY_NODE_URL", "nodes over there")]).unwrap_err();

  assert!(fault.contains("AGENCY_NODE_URL"), "{fault}");

  let fault = read(&[("AGENCY_CACHE_SIREN_URL", "ftp://siren.example/")]).unwrap_err();

  assert!(fault.contains("AGENCY_CACHE_SIREN_URL"), "{fault}");
}

#[test]
fn the_brain_is_the_named_folder_else_the_one_under_the_data_root() {
  let data = Path::new("/data");

  assert_eq!(read(&[]).unwrap().brain_root(data), data.join("brain"));
  assert_eq!(read(&[("AGENCY_BRAIN_ROOT", "  ")]).unwrap().brain_root(data), data.join("brain"));
  assert_eq!(
    read(&[("AGENCY_BRAIN_ROOT", " /srv/brain ")]).unwrap().brain_root(data),
    PathBuf::from("/srv/brain")
  );
}
