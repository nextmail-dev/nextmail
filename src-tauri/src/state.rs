use std::sync::Arc;

use tauri::AppHandle;

use crate::{
    adapters::{
        AccountsStore, AppPaths, BootstrapStore, MailConnectionTester,
        NotificationPreferencesStore, PreferencesStore, ReadingPreferencesStore,
        SystemAttachmentOpener, SystemCredentialStore, SystemExternalLinkOpener,
    },
    application::{AppConfigStores, AppService},
    composer_runtime::ComposerRuntime,
    core::ExternalLinkOpener,
    error::CommandResult,
    mail_runtime::MailRuntime,
    notification_runtime::NotificationRuntime,
    protocols::AsyncImapProvider,
    storage::SqliteMailRepositoryProvider,
};

pub struct AppState {
    pub service: Arc<AppService>,
    pub mail: Arc<MailRuntime>,
    pub composer: Arc<ComposerRuntime>,
    pub notifications: Arc<NotificationRuntime>,
    pub external_link_opener: Arc<dyn ExternalLinkOpener>,
}

impl AppState {
    pub fn from_handle(app: &AppHandle) -> CommandResult<Self> {
        let paths = AppPaths::from_handle(app)?;
        let bootstrap = Arc::new(BootstrapStore::new(&paths));
        let accounts = Arc::new(AccountsStore::new(&paths));
        let preferences = Arc::new(PreferencesStore::new(&paths));
        let reading_preferences = Arc::new(ReadingPreferencesStore::new(&paths));
        let notification_preferences = Arc::new(NotificationPreferencesStore::new(&paths));
        let stores = AppConfigStores::new(
            bootstrap,
            accounts,
            preferences,
            reading_preferences,
            notification_preferences,
        );
        let service = Arc::new(AppService::new(
            paths,
            stores,
            Arc::new(SystemCredentialStore),
            Arc::new(MailConnectionTester),
        ));
        let notifications = Arc::new(NotificationRuntime::new(app.clone(), Arc::clone(&service)));
        let mail = Arc::new(MailRuntime::new(
            app.clone(),
            Arc::clone(&service),
            Arc::new(AsyncImapProvider),
            Arc::new(SqliteMailRepositoryProvider),
            Arc::new(SystemAttachmentOpener),
            Arc::clone(&notifications),
        ));
        let composer = Arc::new(ComposerRuntime::new(
            app.clone(),
            Arc::clone(&service),
            Arc::clone(&mail),
        ));
        let external_link_opener = Arc::new(SystemExternalLinkOpener);
        Ok(Self {
            service,
            mail,
            composer,
            notifications,
            external_link_opener,
        })
    }
}
