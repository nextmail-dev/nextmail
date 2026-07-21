use std::sync::Arc;

use tauri::AppHandle;

use crate::{
    adapters::{
        AccountsStore, AppPaths, BootstrapStore, MailConnectionTester, PreferencesStore,
        ReadingPreferencesStore, SystemAttachmentOpener, SystemCredentialStore,
        SystemExternalLinkOpener,
    },
    application::AppService,
    composer_runtime::ComposerRuntime,
    core::ExternalLinkOpener,
    error::CommandResult,
    mail_runtime::MailRuntime,
    protocols::AsyncImapProvider,
    storage::SqliteMailRepositoryProvider,
};

pub struct AppState {
    pub service: Arc<AppService>,
    pub mail: Arc<MailRuntime>,
    pub composer: Arc<ComposerRuntime>,
    pub external_link_opener: Arc<dyn ExternalLinkOpener>,
}

impl AppState {
    pub fn from_handle(app: &AppHandle) -> CommandResult<Self> {
        let paths = AppPaths::from_handle(app)?;
        let bootstrap = Arc::new(BootstrapStore::new(&paths));
        let accounts = Arc::new(AccountsStore::new(&paths));
        let preferences = Arc::new(PreferencesStore::new(&paths));
        let reading_preferences = Arc::new(ReadingPreferencesStore::new(&paths));
        let service = Arc::new(AppService::new(
            paths,
            bootstrap,
            accounts,
            preferences,
            reading_preferences,
            Arc::new(SystemCredentialStore),
            Arc::new(MailConnectionTester),
        ));
        let mail = Arc::new(MailRuntime::new(
            app.clone(),
            Arc::clone(&service),
            Arc::new(AsyncImapProvider),
            Arc::new(SqliteMailRepositoryProvider),
            Arc::new(SystemAttachmentOpener),
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
            external_link_opener,
        })
    }
}
