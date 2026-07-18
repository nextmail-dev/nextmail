use std::path::Path;

use crate::{
    core::{CommandError, CommandResult},
    storage::PreparedAttachmentFile,
};

pub trait AttachmentOpener: Send + Sync {
    fn open(&self, path: &Path) -> CommandResult<()>;
    fn reveal(&self, path: &Path) -> CommandResult<()>;
}

pub struct SystemAttachmentOpener;

impl AttachmentOpener for SystemAttachmentOpener {
    fn open(&self, path: &Path) -> CommandResult<()> {
        tauri_plugin_opener::open_path(path, None::<&str>)
            .map_err(|_| CommandError::new("attachment.open_failed"))
    }

    fn reveal(&self, path: &Path) -> CommandResult<()> {
        tauri_plugin_opener::reveal_item_in_dir(path)
            .map_err(|_| CommandError::new("attachment.reveal_failed"))
    }
}

pub fn open_prepared_attachment(
    opener: &dyn AttachmentOpener,
    attachment: &PreparedAttachmentFile,
) -> CommandResult<()> {
    if attachment.high_risk {
        opener.reveal(&attachment.path)
    } else {
        opener.open(&attachment.path)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct FakeOpener {
        actions: Mutex<Vec<&'static str>>,
    }

    impl AttachmentOpener for FakeOpener {
        fn open(&self, _path: &Path) -> CommandResult<()> {
            self.actions.lock().unwrap().push("open");
            Ok(())
        }

        fn reveal(&self, _path: &Path) -> CommandResult<()> {
            self.actions.lock().unwrap().push("reveal");
            Ok(())
        }
    }

    #[test]
    fn high_risk_attachments_are_revealed_instead_of_opened() {
        let opener = FakeOpener::default();
        let attachment = PreparedAttachmentFile {
            path: "invoice.exe".into(),
            file_name: "invoice.exe".to_owned(),
            high_risk: true,
        };

        open_prepared_attachment(&opener, &attachment).unwrap();

        assert_eq!(*opener.actions.lock().unwrap(), vec!["reveal"]);
    }

    #[test]
    fn ordinary_attachments_use_the_system_association() {
        let opener = FakeOpener::default();
        let attachment = PreparedAttachmentFile {
            path: "report.pdf".into(),
            file_name: "report.pdf".to_owned(),
            high_risk: false,
        };

        open_prepared_attachment(&opener, &attachment).unwrap();

        assert_eq!(*opener.actions.lock().unwrap(), vec!["open"]);
    }
}
