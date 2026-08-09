use tauri::{AppHandle, Manager};

use crate::core::LanguagePreference;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowTitleKind {
    Main,
    Composer,
    Settings,
    Accounts,
    MessagePreview,
    RawMessage,
    TemplateEditor,
    SignatureEditor,
    Update,
    Notification,
}

pub fn window_title(language: &LanguagePreference, kind: WindowTitleKind) -> &'static str {
    match (language, kind) {
        (_, WindowTitleKind::Main | WindowTitleKind::Notification) => "NextMail",
        (LanguagePreference::ZhCn, WindowTitleKind::Composer) => "写邮件 — NextMail",
        (LanguagePreference::EnUs, WindowTitleKind::Composer) => "Compose — NextMail",
        (LanguagePreference::ZhCn, WindowTitleKind::Settings) => "设置 — NextMail",
        (LanguagePreference::EnUs, WindowTitleKind::Settings) => "Settings — NextMail",
        (LanguagePreference::ZhCn, WindowTitleKind::Accounts) => "账户管理 — NextMail",
        (LanguagePreference::EnUs, WindowTitleKind::Accounts) => "Account Management — NextMail",
        (LanguagePreference::ZhCn, WindowTitleKind::MessagePreview) => "邮件 — NextMail",
        (LanguagePreference::EnUs, WindowTitleKind::MessagePreview) => "Message — NextMail",
        (LanguagePreference::ZhCn, WindowTitleKind::RawMessage) => "邮件原文 — NextMail",
        (LanguagePreference::EnUs, WindowTitleKind::RawMessage) => "Message Source — NextMail",
        (LanguagePreference::ZhCn, WindowTitleKind::TemplateEditor) => "编辑模板 — NextMail",
        (LanguagePreference::EnUs, WindowTitleKind::TemplateEditor) => "Edit Template — NextMail",
        (LanguagePreference::ZhCn, WindowTitleKind::SignatureEditor) => "编辑签名 — NextMail",
        (LanguagePreference::EnUs, WindowTitleKind::SignatureEditor) => "Edit Signature — NextMail",
        (LanguagePreference::ZhCn, WindowTitleKind::Update) => "软件更新 — NextMail",
        (LanguagePreference::EnUs, WindowTitleKind::Update) => "Software Update — NextMail",
    }
}

pub fn update_open_window_titles(app: &AppHandle, language: &LanguagePreference) {
    for (label, window) in app.webview_windows() {
        let kind = kind_for_label(&label);
        if let Err(error) = window.set_title(window_title(language, kind)) {
            tracing::warn!(%label, ?error, "window title update failed");
        }
    }
}

fn kind_for_label(label: &str) -> WindowTitleKind {
    if label.starts_with("composer-") {
        WindowTitleKind::Composer
    } else if label.starts_with("message-preview-") {
        WindowTitleKind::MessagePreview
    } else if label.starts_with("definition-template-") {
        WindowTitleKind::TemplateEditor
    } else if label.starts_with("definition-signature-") {
        WindowTitleKind::SignatureEditor
    } else if label.starts_with("notification-") {
        WindowTitleKind::Notification
    } else {
        match label {
            "settings" => WindowTitleKind::Settings,
            "accounts" => WindowTitleKind::Accounts,
            "raw-message" => WindowTitleKind::RawMessage,
            "update" => WindowTitleKind::Update,
            _ => WindowTitleKind::Main,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localizes_every_business_window_title() {
        assert_eq!(
            window_title(&LanguagePreference::ZhCn, WindowTitleKind::Composer),
            "写邮件 — NextMail"
        );
        assert_eq!(
            window_title(&LanguagePreference::EnUs, WindowTitleKind::Settings),
            "Settings — NextMail"
        );
        assert_eq!(
            kind_for_label("definition-signature-global-new"),
            WindowTitleKind::SignatureEditor
        );
        assert_eq!(kind_for_label("raw-message"), WindowTitleKind::RawMessage);
        assert_eq!(
            kind_for_label("message-preview-123"),
            WindowTitleKind::MessagePreview
        );
        assert_eq!(
            kind_for_label("notification-123"),
            WindowTitleKind::Notification
        );
        assert_eq!(kind_for_label("update"), WindowTitleKind::Update);
    }
}
