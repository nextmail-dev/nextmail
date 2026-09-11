use crate::core::{CommandResult, ExternalLinkOpener};

pub struct SystemExternalLinkOpener;

impl ExternalLinkOpener for SystemExternalLinkOpener {
    fn open(&self, target: &str) -> CommandResult<()> {
        tauri_plugin_opener::open_url(target, None::<&str>).map_err(|error| {
            crate::diagnostics::command_error("message.link_open_failed", false, &error)
        })
    }
}
