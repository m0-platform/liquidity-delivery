pub mod chains;
pub mod svm;

pub use chains::*;
pub use svm::*;

pub fn unix_timestamp_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_secs()
}
