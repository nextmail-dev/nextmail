use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalRect, WebviewUrl,
    WebviewWindowBuilder,
};
use uuid::Uuid;

use crate::{
    application::AppService,
    core::{
        CommandError, CommandResult, NewMailCandidate, NewMailNotification,
        NotificationDisplayMode, NotificationPreferences,
    },
};

const NOTIFICATION_WIDTH: f64 = 360.0;
const NOTIFICATION_HEIGHT: f64 = 116.0;
const NOTIFICATION_MARGIN: f64 = 16.0;
const NOTIFICATION_GAP: f64 = 10.0;

pub struct NotificationRuntime {
    app: AppHandle,
    service: Arc<AppService>,
    state: Mutex<NotificationState>,
}

#[derive(Default)]
struct NotificationState {
    active: Vec<ActiveNotification>,
    next_generation: u64,
}

#[derive(Clone)]
struct ActiveNotification {
    notification: NewMailNotification,
    generation: u64,
}

struct PresentResult {
    notification: NewMailNotification,
    evicted_ids: Vec<String>,
    generation: u64,
}

impl NotificationRuntime {
    pub fn new(app: AppHandle, service: Arc<AppService>) -> Self {
        Self {
            app,
            service,
            state: Mutex::new(NotificationState::default()),
        }
    }

    pub fn present(
        self: &Arc<Self>,
        candidate: NewMailCandidate,
        preferences: &NotificationPreferences,
    ) {
        let Ok(account) = self.service.account_record(&candidate.account_id) else {
            return;
        };
        let notification = NewMailNotification {
            id: Uuid::new_v4().to_string(),
            account_id: candidate.account_id,
            account_name: account.display_name,
            account_email: account.email,
            mailbox_id: candidate.mailbox_id,
            message_id: candidate.message_id,
            sender_name: candidate.sender_name,
            sender_email: candidate.sender_email,
            subject: candidate.subject,
        };
        let max_visible = self.max_visible(preferences.max_stacked);
        let result = self
            .state()
            .present(notification, &preferences.display_mode, max_visible);

        self.destroy_ids(result.evicted_ids);
        if self.ensure_window(&result.notification).is_err() {
            self.remove_without_destroy(&result.notification.id);
            return;
        }
        let _ = self.app.emit_to(
            notification_window_label(&result.notification.id),
            "notification-content-changed",
            &result.notification,
        );
        self.reflow();
        if let Some(window) = self
            .app
            .get_webview_window(&notification_window_label(&result.notification.id))
        {
            let _ = window.show();
        }

        let runtime = Arc::clone(self);
        let notification_id = result.notification.id;
        let duration = Duration::from_secs(u64::from(preferences.display_duration_seconds));
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(duration).await;
            runtime.expire(&notification_id, result.generation);
        });
    }

    pub fn bootstrap_for_window(
        &self,
        notification_id: &str,
        window_label: &str,
    ) -> CommandResult<NewMailNotification> {
        ensure_notification_window(notification_id, window_label)?;
        self.state()
            .active
            .iter()
            .find(|active| active.notification.id == notification_id)
            .map(|active| active.notification.clone())
            .ok_or_else(|| CommandError::new("notification.not_found"))
    }

    pub fn take_for_window(
        &self,
        notification_id: &str,
        window_label: &str,
    ) -> CommandResult<NewMailNotification> {
        ensure_notification_window(notification_id, window_label)?;
        let notification = self
            .state()
            .remove(notification_id)
            .map(|active| active.notification)
            .ok_or_else(|| CommandError::new("notification.not_found"))?;
        self.destroy_ids(vec![notification_id.to_owned()]);
        self.reflow();
        Ok(notification)
    }

    pub fn dismiss_for_window(
        &self,
        notification_id: &str,
        window_label: &str,
    ) -> CommandResult<()> {
        let _ = self.take_for_window(notification_id, window_label)?;
        Ok(())
    }

    pub fn preferences_changed(&self) {
        self.dismiss_all();
    }

    pub fn dismiss_account(&self, account_id: &str) {
        let removed = self.state().remove_account(account_id);
        self.destroy_ids(removed);
        self.reflow();
    }

    fn expire(&self, notification_id: &str, generation: u64) {
        let removed = self
            .state()
            .remove_if_generation(notification_id, generation)
            .is_some();
        if removed {
            self.destroy_ids(vec![notification_id.to_owned()]);
            self.reflow();
        }
    }

    fn dismiss_all(&self) {
        let removed = self.state().drain_ids();
        self.destroy_ids(removed);
    }

    fn remove_without_destroy(&self, notification_id: &str) {
        let _ = self.state().remove(notification_id);
        self.reflow();
    }

    fn ensure_window(&self, notification: &NewMailNotification) -> CommandResult<()> {
        let label = notification_window_label(&notification.id);
        if self.app.get_webview_window(&label).is_some() {
            return Ok(());
        }
        let url = format!(
            "index.html?window=notification&notificationId={}",
            notification.id
        );
        WebviewWindowBuilder::new(&self.app, &label, WebviewUrl::App(url.into()))
            .title("NextMail")
            .inner_size(NOTIFICATION_WIDTH, NOTIFICATION_HEIGHT)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .closable(false)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false)
            .focusable(false)
            .shadow(true)
            .visible(false)
            .build()
            .map(|_| ())
            .map_err(|_| CommandError::new("notification.window_create_failed"))
    }

    fn reflow(&self) {
        let ids = self.state().active_ids();
        let Some(monitor) = self.notification_monitor() else {
            return;
        };
        let work_area = *monitor.work_area();
        let scale_factor = monitor.scale_factor();
        for (index, id) in ids.iter().enumerate() {
            let slot_from_bottom = ids.len().saturating_sub(index + 1);
            if let Some(window) = self.app.get_webview_window(&notification_window_label(id)) {
                let position = notification_position(&work_area, scale_factor, slot_from_bottom);
                let _ = window.set_position(position);
            }
        }
    }

    fn max_visible(&self, configured: u8) -> usize {
        self.notification_monitor()
            .map(|monitor| {
                usize::from(configured)
                    .min(notification_capacity(
                        monitor.work_area().size.height,
                        monitor.scale_factor(),
                    ))
                    .max(1)
            })
            .unwrap_or_else(|| usize::from(configured).max(1))
    }

    fn notification_monitor(&self) -> Option<Monitor> {
        self.app
            .get_webview_window("main")
            .and_then(|window| window.current_monitor().ok().flatten())
            .or_else(|| self.app.primary_monitor().ok().flatten())
    }

    fn destroy_ids(&self, ids: Vec<String>) {
        for id in ids {
            if let Some(window) = self.app.get_webview_window(&notification_window_label(&id)) {
                let _ = window.destroy();
            }
        }
    }

    fn state(&self) -> MutexGuard<'_, NotificationState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl NotificationState {
    fn present(
        &mut self,
        mut notification: NewMailNotification,
        display_mode: &NotificationDisplayMode,
        max_visible: usize,
    ) -> PresentResult {
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let generation = self.next_generation;
        let mut evicted_ids = Vec::new();

        if *display_mode == NotificationDisplayMode::Replace && !self.active.is_empty() {
            let mut active = self.active.pop().expect("active notification must exist");
            notification.id.clone_from(&active.notification.id);
            active.notification = notification.clone();
            active.generation = generation;
            evicted_ids.extend(self.active.drain(..).map(|removed| removed.notification.id));
            self.active.push(active);
        } else {
            self.active.push(ActiveNotification {
                notification: notification.clone(),
                generation,
            });
            while self.active.len() > max_visible.max(1) {
                evicted_ids.push(self.active.remove(0).notification.id);
            }
        }

        PresentResult {
            notification,
            evicted_ids,
            generation,
        }
    }

    fn remove(&mut self, notification_id: &str) -> Option<ActiveNotification> {
        self.active
            .iter()
            .position(|active| active.notification.id == notification_id)
            .map(|index| self.active.remove(index))
    }

    fn remove_if_generation(
        &mut self,
        notification_id: &str,
        generation: u64,
    ) -> Option<ActiveNotification> {
        self.active
            .iter()
            .position(|active| {
                active.notification.id == notification_id && active.generation == generation
            })
            .map(|index| self.active.remove(index))
    }

    fn remove_account(&mut self, account_id: &str) -> Vec<String> {
        let mut removed = Vec::new();
        self.active.retain(|active| {
            if active.notification.account_id == account_id {
                removed.push(active.notification.id.clone());
                false
            } else {
                true
            }
        });
        removed
    }

    fn drain_ids(&mut self) -> Vec<String> {
        self.active
            .drain(..)
            .map(|active| active.notification.id)
            .collect()
    }

    fn active_ids(&self) -> Vec<String> {
        self.active
            .iter()
            .map(|active| active.notification.id.clone())
            .collect()
    }
}

fn ensure_notification_window(notification_id: &str, window_label: &str) -> CommandResult<()> {
    if notification_window_label(notification_id) == window_label {
        Ok(())
    } else {
        Err(CommandError::new("notification.window_invalid"))
    }
}

fn notification_window_label(notification_id: &str) -> String {
    format!("notification-{notification_id}")
}

fn notification_capacity(work_area_height: u32, scale_factor: f64) -> usize {
    let height = logical_to_physical(NOTIFICATION_HEIGHT, scale_factor);
    let margin = logical_to_physical(NOTIFICATION_MARGIN, scale_factor);
    let gap = logical_to_physical(NOTIFICATION_GAP, scale_factor);
    let available = i64::from(work_area_height).saturating_sub(margin * 2);
    ((available + gap) / (height + gap)).max(1) as usize
}

fn notification_position(
    work_area: &PhysicalRect<i32, u32>,
    scale_factor: f64,
    slot_from_bottom: usize,
) -> PhysicalPosition<i32> {
    let width = logical_to_physical(NOTIFICATION_WIDTH, scale_factor);
    let height = logical_to_physical(NOTIFICATION_HEIGHT, scale_factor);
    let margin = logical_to_physical(NOTIFICATION_MARGIN, scale_factor);
    let gap = logical_to_physical(NOTIFICATION_GAP, scale_factor);
    let left = i64::from(work_area.position.x);
    let top = i64::from(work_area.position.y);
    let right = left + i64::from(work_area.size.width);
    let bottom = top + i64::from(work_area.size.height);
    let slot = i64::try_from(slot_from_bottom).unwrap_or(i64::MAX);
    let x = (right - width - margin).max(left + margin);
    let y = (bottom - height - margin - slot.saturating_mul(height + gap)).max(top + margin);
    PhysicalPosition::new(clamp_i64_to_i32(x), clamp_i64_to_i32(y))
}

fn logical_to_physical(value: f64, scale_factor: f64) -> i64 {
    (value * scale_factor).round().max(1.0) as i64
}

fn clamp_i64_to_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::{
        notification_capacity, notification_position, NotificationDisplayMode, NotificationState,
    };
    use crate::core::NewMailNotification;
    use tauri::{PhysicalPosition, PhysicalRect, PhysicalSize};

    fn notification(id: &str, account_id: &str, subject: &str) -> NewMailNotification {
        NewMailNotification {
            id: id.to_owned(),
            account_id: account_id.to_owned(),
            account_name: "Account".to_owned(),
            account_email: "account@example.com".to_owned(),
            mailbox_id: "inbox".to_owned(),
            message_id: format!("message-{id}"),
            sender_name: Some("Sender".to_owned()),
            sender_email: "sender@example.com".to_owned(),
            subject: subject.to_owned(),
        }
    }

    #[test]
    fn stacked_notifications_evict_the_oldest_at_the_visible_limit() {
        let mut state = NotificationState::default();
        state.present(
            notification("one", "account", "One"),
            &NotificationDisplayMode::Stacked,
            2,
        );
        state.present(
            notification("two", "account", "Two"),
            &NotificationDisplayMode::Stacked,
            2,
        );
        let result = state.present(
            notification("three", "account", "Three"),
            &NotificationDisplayMode::Stacked,
            2,
        );
        assert_eq!(result.evicted_ids, vec!["one"]);
        assert_eq!(state.active_ids(), vec!["two", "three"]);
    }

    #[test]
    fn replace_mode_reuses_one_window_and_invalidates_the_old_timer() {
        let mut state = NotificationState::default();
        let first = state.present(
            notification("one", "account", "One"),
            &NotificationDisplayMode::Replace,
            3,
        );
        let replacement = state.present(
            notification("two", "account", "Two"),
            &NotificationDisplayMode::Replace,
            3,
        );
        assert_eq!(replacement.notification.id, first.notification.id);
        assert_eq!(replacement.notification.subject, "Two");
        assert!(state
            .remove_if_generation(&first.notification.id, first.generation)
            .is_none());
        assert!(state
            .remove_if_generation(&replacement.notification.id, replacement.generation)
            .is_some());
    }

    #[test]
    fn layout_uses_the_monitor_work_area_and_scales_upward() {
        let work_area = PhysicalRect {
            position: PhysicalPosition::new(1920, 40),
            size: PhysicalSize::new(2560, 1400),
        };
        let bottom = notification_position(&work_area, 2.0, 0);
        let above = notification_position(&work_area, 2.0, 1);
        assert_eq!(bottom.x, 3728);
        assert_eq!(bottom.y, 1176);
        assert_eq!(above.y, 924);
        assert_eq!(notification_capacity(1400, 2.0), 5);
    }

    #[test]
    fn account_removal_only_dismisses_matching_notifications() {
        let mut state = NotificationState::default();
        state.present(
            notification("one", "a", "One"),
            &NotificationDisplayMode::Stacked,
            3,
        );
        state.present(
            notification("two", "b", "Two"),
            &NotificationDisplayMode::Stacked,
            3,
        );
        assert_eq!(state.remove_account("a"), vec!["one"]);
        assert_eq!(state.active_ids(), vec!["two"]);
    }
}
