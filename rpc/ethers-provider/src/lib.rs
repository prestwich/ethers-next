#![warn(
    missing_debug_implementations,
    // missing_docs,
    unreachable_pub,
    unused_crate_dependencies
)]
#![deny(unused_must_use, rust_2018_idioms)]
#![doc(test(
    no_crate_inject,
    attr(deny(warnings, rust_2018_idioms), allow(dead_code, unused_variables))
))]

pub mod provider;
pub use provider::{HttpProvider, Provider};

pub mod quorum;
pub mod retry;
pub mod rw;

use std::time::Duration;
// The default polling interval for filters and pending transactions
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(7000);

/// The polling interval to use for local endpoints, See [`ethers_transports::Connection::is_local()`]
pub const DEFAULT_LOCAL_POLL_INTERVAL: Duration = Duration::from_millis(100);
