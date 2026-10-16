//! The model gateway's tests: the table written from the registry, and the gateway's wire contract
//! asked of its binary over a scratch store.

#[path = "../support/binary.rs"]
mod binary;
#[path = "../support/scratch.rs"]
mod scratch;

mod contract;
mod table;
