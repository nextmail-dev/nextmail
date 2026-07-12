use std::sync::Arc;

use tauri::AppHandle;

use crate::{
    adapters::{AppPaths, MailConnectionTester, SystemCredentialStore},
    application::AppService,
    composer_runtime::ComposerRuntime,
    error::CommandResult,
    mail_runtime::MailRuntime,
};

pub struct AppState {
    pub service: Arc<AppService>,
    pub mail: Arc<MailRuntime>,
    pub composer: Arc<ComposerRuntime>,
}

impl AppState {
    pub fn from_handle(app: &AppHandle) -> CommandResult<Self> {
        let paths = AppPaths::from_handle(app)?;
        let service = Arc::new(AppService::new(
            paths,
            Arc::new(SystemCredentialStore),
            Arc::new(MailConnectionTester),
        ));
        let mail = Arc::new(MailRuntime::new(app.clone(), Arc::clone(&service)));
        let composer = Arc::new(ComposerRuntime::new(app.clone(), Arc::clone(&service)));
        Ok(Self {
            service,
            mail,
            composer,
        })
    }
}
