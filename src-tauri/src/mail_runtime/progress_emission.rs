use crate::core::SyncProgress;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct ProgressEmission {
    last: Option<(Instant, crate::core::SyncPhase, Option<String>)>,
}

impl ProgressEmission {
    pub(super) fn should_emit(&mut self, progress: &SyncProgress, now: Instant) -> bool {
        // Keep the authoritative snapshot current on every message, but do not
        // enqueue thousands of WebView evaluations before JS can coalesce them.
        let changed = self.last.as_ref().is_none_or(|(last, phase, mailbox)| {
            phase != &progress.phase
                || mailbox != &progress.current_mailbox_name
                || now.duration_since(*last) >= Duration::from_millis(100)
        });
        if changed || progress.error_code.is_some() {
            self.last = Some((
                now,
                progress.phase.clone(),
                progress.current_mailbox_name.clone(),
            ));
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SyncPhase;
    #[test]
    fn bounds_progress_bridge_traffic_and_emits_completion_immediately() {
        let mut emission = ProgressEmission::default();
        let start = Instant::now();
        let mut progress = SyncProgress {
            account_id: "account".into(),
            phase: SyncPhase::Summaries,
            completed: 0,
            total: 10_000,
            current_mailbox_name: Some("Inbox".into()),
            error_code: None,
            revision: 1,
        };
        let mut sent = 0;
        for index in 0..10_000 {
            progress.completed = index;
            if emission.should_emit(&progress, start + Duration::from_micros(index * 10)) {
                sent += 1;
            }
        }
        assert_eq!(sent, 1);
        progress.phase = SyncPhase::Complete;
        assert!(emission.should_emit(&progress, start + Duration::from_millis(100)));
        progress.error_code = Some("sync.failed".into());
        assert!(emission.should_emit(&progress, start + Duration::from_millis(101)));
    }
}
