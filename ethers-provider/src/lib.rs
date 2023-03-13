pub mod provider;
pub use provider::{HttpProvider, Provider};

pub mod quorum;
pub mod retry;
pub mod rw;

use std::time::Duration;
// The default polling interval for filters and pending transactions
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(7000);

/// The polling interval to use for local endpoints, See [`crate::is_local_endpoint()`]
pub const DEFAULT_LOCAL_POLL_INTERVAL: Duration = Duration::from_millis(100);
