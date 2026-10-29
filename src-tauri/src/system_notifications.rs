use tauri::{AppHandle, Manager};
use tauri_plugin_notification::{ActionPerformed, NotificationExt, PermissionState};

use crate::core::{
    CommandError, CommandResult, LanguagePreference, NewMailCandidate, SystemNotificationMode,
};

const TARGET_KEY: &str = "nextmailTarget";

pub fn setup(app: &AppHandle) -> CommandResult<()> {
    let action_app = app.clone();
    app.notification()
        .on_action(move |action| {
            if let Some(token) = action_target(action) {
                activate(&action_app, token);
            }
        })
        .map_err(|error| {
            crate::diagnostics::command_error("notification.system_send_failed", false, &error)
        })
}

fn action_target(action: &ActionPerformed) -> Option<&str> {
    if action.action_id() != "tap" {
        return None;
    }
    action.notification()?.extra().get(TARGET_KEY)?.as_str()
}

pub fn ensure_permission(app: &AppHandle) -> CommandResult<()> {
    let permission = app.notification().request_permission().map_err(|error| {
        crate::diagnostics::command_error("notification.system_permission_failed", false, &error)
    })?;
    if permission != PermissionState::Granted {
        return Err(CommandError::new("notification.system_permission_denied"));
    }
    Ok(())
}

pub fn present(app: &AppHandle, candidates: &[NewMailCandidate], mode: &SystemNotificationMode) {
    let state = app.state::<crate::state::AppState>();
    if state.demo.active() {
        return;
    }
    let Ok(preferences) = state.service.get_preferences() else {
        return;
    };
    let Some(last) = candidates.last() else {
        return;
    };
    let Ok(account) = state.service.account_record(&last.account_id) else {
        return;
    };
    let account_label = if account.display_name.is_empty() {
        &account.email
    } else {
        &account.display_name
    };
    let Some((title, body)) = content(candidates, mode, &preferences.language, account_label)
    else {
        return;
    };
    let token = state.notifications.register_system_target(last);
    // Only an opaque token goes through the OS callback. Resolve the trusted
    // location once in Rust instead of accepting location IDs from its payload.
    let result = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .extra(TARGET_KEY, &token)
        .show();
    if let Err(error) = result {
        let _ = state.notifications.take_system_target(&token);
        let _ = crate::diagnostics::command_error("notification.system_send_failed", true, &error);
    }
}

fn activate(app: &AppHandle, token: &str) {
    let state = app.state::<crate::state::AppState>();
    if state.demo.active() {
        return;
    }
    let Some(candidate) = state.notifications.take_system_target(token) else {
        return;
    };
    let Ok(preferences) = state.service.get_notification_preferences() else {
        return;
    };
    if !preferences.enabled
        || !preferences.use_system_notifications
        || !preferences.account_enabled(&candidate.account_id)
    {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        crate::notification_runtime::activate_candidate(&app, &candidate).await;
    });
}

fn text(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(256)
        .map(|c| match c {
            '<' => '‹',
            '>' => '›',
            '&' => '＆',
            _ => c,
        })
        .collect()
}

fn content(
    candidates: &[NewMailCandidate],
    mode: &SystemNotificationMode,
    language: &LanguagePreference,
    account: &str,
) -> Option<(String, String)> {
    let last = candidates.last()?;
    let chinese = *language == LanguagePreference::ZhCn;
    if *mode == SystemNotificationMode::Count && candidates.len() > 1 {
        let title = if chinese {
            format!("收到 {} 封新邮件", candidates.len())
        } else {
            format!("{} new messages", candidates.len())
        };
        return Some((title, text(account)));
    }
    let sender = last
        .sender_name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(&last.sender_email);
    let subject = if last.subject.trim().is_empty() {
        if chinese {
            "（无主题）"
        } else {
            "(No subject)"
        }
    } else {
        &last.subject
    };
    let title = if chinese {
        format!("新邮件 · {}", text(account))
    } else {
        format!("New mail · {}", text(account))
    };
    Some((title, format!("{}: {}", text(sender), text(subject))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_tap_with_our_string_token_requests_navigation() {
        for (action_id, notification, expected) in [
            (
                "tap",
                serde_json::json!({"id": 1, "extra": {"nextmailTarget": "token"}}),
                Some("token"),
            ),
            (
                "dismiss",
                serde_json::json!({"id": 1, "extra": {"nextmailTarget": "token"}}),
                None,
            ),
            (
                "other",
                serde_json::json!({"id": 1, "extra": {"nextmailTarget": "token"}}),
                None,
            ),
            ("tap", serde_json::Value::Null, None),
            ("tap", serde_json::json!({"id": 1}), None),
            (
                "tap",
                serde_json::json!({"id": 1, "extra": {"nextmailTarget": 123}}),
                None,
            ),
            (
                "tap",
                serde_json::json!({"id": 1, "extra": {"accountId": "other", "messageId": "other"}}),
                None,
            ),
        ] {
            let action: ActionPerformed = serde_json::from_value(serde_json::json!({
                "actionId": action_id,
                "notification": notification,
            }))
            .unwrap();
            assert_eq!(action_target(&action), expected);
        }
    }

    fn candidate(subject: &str) -> NewMailCandidate {
        NewMailCandidate {
            account_id: "a".into(),
            mailbox_id: "inbox".into(),
            message_id: subject.into(),
            sender_name: Some("Sender".into()),
            sender_email: "sender@example.com".into(),
            subject: subject.into(),
        }
    }
    #[test]
    fn count_uses_full_batch_and_latest_only_uses_last_message() {
        let messages = (0..25)
            .map(|i| candidate(&format!("Subject {i}")))
            .collect::<Vec<_>>();
        assert_eq!(
            content(
                &messages,
                &SystemNotificationMode::Count,
                &LanguagePreference::EnUs,
                "Account"
            ),
            Some(("25 new messages".into(), "Account".into()))
        );
        let latest = content(
            &messages,
            &SystemNotificationMode::Latest,
            &LanguagePreference::ZhCn,
            "账户",
        )
        .unwrap();
        assert_eq!(latest.1, "Sender: Subject 24");
        assert!(!latest.1.contains("Subject 0"));
        assert!(content(
            &[],
            &SystemNotificationMode::Count,
            &LanguagePreference::EnUs,
            "Account"
        )
        .is_none());
    }
    #[test]
    fn single_mail_shows_summary_and_untrusted_text_stays_bounded() {
        let item = candidate("");
        let summary = content(
            &[item],
            &SystemNotificationMode::Count,
            &LanguagePreference::EnUs,
            "Account",
        )
        .unwrap();
        assert_eq!(summary.1, "Sender: (No subject)");
        assert_eq!(text("<b>\n&"), "‹b›＆");
        assert_eq!(text(&"x".repeat(500)).len(), 256);
    }
}
