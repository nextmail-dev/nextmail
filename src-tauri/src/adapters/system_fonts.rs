use std::{collections::BTreeMap, sync::OnceLock};

use crate::core::SystemFontProvider;

#[derive(Default)]
pub struct InstalledSystemFonts {
    families: OnceLock<Vec<String>>,
}

impl SystemFontProvider for InstalledSystemFonts {
    fn families(&self) -> Vec<String> {
        self.families
            .get_or_init(|| {
                let mut database = fontdb::Database::new();
                database.load_system_fonts();
                // Keep names only. Drop parsed font data as soon as enumeration finishes.
                normalize_families(
                    database
                        .faces()
                        .flat_map(|face| face.families.iter().map(|(name, _)| name.as_str())),
                )
            })
            .clone()
    }
}

fn normalize_families<'a>(names: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut families = BTreeMap::new();
    for name in names {
        let name = name.trim();
        if name.is_empty()
            || name.len() > 256
            || name.chars().any(char::is_control)
            || name.starts_with('@')
        {
            continue;
        }
        families
            .entry(name.to_lowercase())
            .or_insert_with(|| name.to_owned());
    }
    families.into_values().take(4096).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_names_are_deduplicated_and_bounded() {
        assert_eq!(
            normalize_families([" Arial ", "arial", "", "@Arial", "bad\nname", "宋体"].into_iter()),
            vec!["Arial", "宋体"]
        );
        assert!(normalize_families(["x".repeat(257)].iter().map(String::as_str)).is_empty());
    }
}
