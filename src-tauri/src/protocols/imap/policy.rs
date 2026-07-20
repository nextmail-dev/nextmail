use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::core::SyncPolicy;

pub(super) fn should_download_body(policy: SyncPolicy, received_at: i64) -> bool {
    sync_policy_cutoff(policy).is_none_or(|cutoff| received_at >= cutoff)
}

pub(super) fn sync_policy_cutoff(policy: SyncPolicy) -> Option<i64> {
    let days = match policy {
        SyncPolicy::Days30 => 30,
        SyncPolicy::Days90 => 90,
        SyncPolicy::Days365 => 365,
        SyncPolicy::All => return None,
    };
    Some(now().saturating_sub(Duration::from_secs(days * 86_400).as_secs() as i64))
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_the_configured_download_window() {
        assert!(should_download_body(SyncPolicy::All, 0));
        assert!(should_download_body(SyncPolicy::Days30, now()));
        assert!(!should_download_body(SyncPolicy::Days30, 0));
    }
}
