use crate::core::{CommandError, CommandResult, ExternalLinkOpener};

pub struct SystemExternalLinkOpener;

impl ExternalLinkOpener for SystemExternalLinkOpener {
    fn open(&self, target: &str) -> CommandResult<()> {
        tauri_plugin_opener::open_url(target, None::<&str>)
            .map_err(|_| CommandError::new("message.link_open_failed"))
    }
}
