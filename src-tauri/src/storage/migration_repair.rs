//! One-time repair for databases migrated by Windows builds of v0.6.5 and
//! earlier. Before `.gitattributes` pinned migration files to LF, Windows
//! checkouts (local and CI, `core.autocrlf=true`) embedded CRLF-based SHA-384
//! checksums via `include_str!`; LF-based binaries (v0.6.6+, and CI builds of
//! all platforms) reject those databases. Rewriting the known CRLF variants to
//! their LF values before `MIGRATOR.run` keeps pre-0.6.6 Windows databases
//! openable. The update only touches rows whose checksum exactly matches a
//! known CRLF digest, is idempotent, and can be removed in a future major
//! version once v0.6.5 databases are out of circulation.

use sqlx::SqlitePool;

/// (version, CRLF-content checksum hex, LF-content checksum hex).
/// Generated from the migration files: `sha384(file.with(\r\n))` /
/// `sha384(file.as_checked_out)`, stored in `_sqlx_migrations.checksum` as
/// raw 48-byte digests (hex below decodes at runtime).
const CRLF_TO_LF_CHECKSUMS: &[(i64, &str, &str)] = &[
    (1, "45968507037a09577db1dc963c59522bf0e8efadbffd95b2a66eb4826d67404ea6646b379bb83870bcee3ee57f458b9c", "d68408bf37fc52f6da8340630549d1be87230caedc7efb889eebc08f961aab05a3a520cdd00911eb00405a965cc163ad"), // 0001_bootstrap.sql
    (2, "04992a4df355a0ff8f97dee04cb946924c6534ef49918cc6799aed1375204d6c64f2caa2d7ff5ecbcebef3f590e32ec8", "5d95bc9d22ca77f4be1a0328ea2f212e897df47d00f272fafd3da4f86e08eeabe1926947a146a3c757a7ed5e28b576de"), // 0002_imap_reading.sql
    (3, "54b961d2b7bc94cd621c77f5d8d25bdbb32c489e32978630e8798fc77e731188ed625f54093c682637ce8e4a041353ff", "13d597463293eb5289fedec22608b1b39759cdb698e0145f5398f201ba21aa482becaabe7bc68a6548604ec9fdd16d90"), // 0003_message_encodings.sql
    (4, "c835b1d02e47769964d2191291e702d41f30382d03829715c59ff218cc7c5423ecb5081cdc78832b57bf09a753ed029e", "b39e0c08c900618394287c29fb1d75b264924cbdb1821eef37aa0ba8bc1d95c04589f75374eb725b02c23d2b1086acb5"), // 0004_compose_and_send.sql
    (5, "1911d1c18c3f16b80857f57327331ce89e628b3370a28f4ebb5c20c366a50ea66446968b5d35f58a0a03f7ae13a0eea8", "b08ce1d7bf137844aa126847c34b8e0aabe44350c6c4ed9e0ada24c64914dbeac06366a29fd06ce595a66771bb4eecb2"), // 0005_complete_imap_sync.sql
    (6, "eaaf6796ac161e889cda4b404efef7a57574f75760d67859fa1cf83f1734e314811d46b5b796e8d1a72652d427905850", "c9a88d907ed34fe570eb09fbe0d79c563117d99c5ca28b7246fd91f8377debf4b85751336270e93a196aec46b9e4a1ae"), // 0006_reply_forward.sql
    (7, "2a948f8b614026f725bb19d640258825848bb6b416b1783aa6f1b1cd095ef97c802880f49a4d8a6947bbff40ee12789f", "57b48c9f719fc6d6451dd42f6b4463a164906a849a2c951ded9a3f269fb02b14fab496b8ca671313ea19dbb01251a561"), // 0007_html_style_fidelity.sql
    (8, "8c3dda8d9b428e79d0193521dccd11c789e2f2988d14ee6d25a0f6315eaa3b46dd8026f3cf3f1238c6019b8d10331245", "59e14fe8aabf32fd0aba54d7d9408d56a04e7fdceeb8cb33885847525f8eff1e079b3083b36461eaec005155ccf259c8"), // 0008_template_signature_library.sql
    (9, "d61aa46f1cc937730689689198551556c3c547efee2957199c0d4819c82f3118bec550ae6b629b3da4aebaf958e7272b", "f35e5709468c99e51815868c0ec5ee71fc6aa81768a8b7566ecd935c2f0642523ea49a9c3b4ca5ef48886c59fbd1b3a8"), // 0009_composition_scene_rules.sql
    (10, "fdaa8ef77b82ab006e3020d87ce6132f58ee7ba57c469cbe795c2dc99e324c1bef69040ee2c7ff899b660c738c251536", "5ef0e09d201e056df4fe841dd41ffca463e90fe7c184c475d31239ad52fdd735d5ea8fd493bde01331fccff27f495a7e"), // 0010_html_stylesheet_and_theme_fidelity.sql
    (11, "f877df8704b7ffc4c3427bcddb3d58b140b1098012b17ba222194dd459c658f058b95876ef1ee41e6c1782076bd3e1d7", "5bd7d70c8e8943069380e2f3ff38e6bd810f4a4565e660e948bffd1650f3b7b5b863d510ada690621eef83c907a942eb"), // 0011_controlled_mail_links.sql
    (12, "d2b43027bedacbcb8bc581cedc9f22d6321ffd04212afa0eaa1beaade9e7663b7119234c6c6cd7815345af2d5572ea72", "1cfa8a4547e82647b0b9cc0bcfa5bce6d39a86ebb88ebdba9b7ec35cc96c35b852aac31a451be225178fe63820e8fb27"), // 0012_direct_mail_links_and_layout_fidelity.sql
    (13, "edc9f4c3a06ff6c18230bf86f32c736cfbefdcdfc145a49d3c94ae48e1bf9ed784bb8d1fcc3d710b67b57c9da7e24f5a", "e71ca2a09ea058b35f0cc8d01e847e0434c773c2a156c5e9487915e334c354428c08cf9ede7eb363167b09750ecc0cc0"), // 0013_composer_inline_images.sql
    (14, "c3c407cd483c7a892ed2e374ca6a5890550ee097162cac7b8255d82e86c2f39875f2342fbc8a40a8099858a8709815a2", "6ff4a2f48bc410acfc1d1df493ca0ce344dc1e4d06496f75aa0f7536fe2e468107557722e950e09b17fcbd6d68611415"), // 0014_functional_selector_fidelity.sql
    (15, "ed7fae97fdeae2d969976c806220b78b4cdd7cf2be41bcb2536da6a0aa4e4a203ea9911e09cb9a6abf5005e8a2d7bc3f", "1e7aed39a188554f5f90f293dd389091ecf31fce0da173c52e147f57af9900d2270be8a642c109dc9af9fda43595d43f"), // 0015_local_message_search.sql
    (16, "7ce00ef9c2a781d053112204b2dcc76c11a4b4b9e191e28b944f662233ca7263965ca444fe0d27792e4dc6ef99e8469e", "0ae76ff2d22f1acbc571a85caf6a62efd177186c1c2337ddd5f971519d1189aee3d46446eeacf1f74b527b9521e002a0"), // 0016_stage12_experience.sql
    (17, "aba96732c4543ca39c002a371fcf9ace1b299bdace87c51b12768a593deae91405795b9b746ae95db5595384f5c450d0", "9475be269776e2d7dfc6facff2a0b7cdee0adde80cd225a2e2312a09389b4890d6a22fa91141d97196b739461369c4ea"), // 0017_signature_preferences.sql
    (18, "023ce07c864f6c95583e2316935ff2aff60099afa4ed2cce3d8831cc33092d838def87a0ed5bb69b8b2e102eecfeb875", "4d40e550b02591c84fa35f06feb624f090eeb3644fe87c967ac3505275a5203907adeac4b05c8344b1a9fed04dbe9287"), // 0018_notification_baseline.sql
    (19, "636242a8ee62af0e6a74bcced452f593107b9ef30c5894bd4456b5c9c0bb19864c2287d1a38790e2451cde3364d12fe8", "43dd27c830187fe6cd5aed182da3c2ed963a52e0ccd9e2ee453684d8fe51bdda088b6d07e56a44780288dd7eb3239dc2"), // 0019_account_sync_interval.sql
    (20, "4b74b8ed0694cdaa522f40f017bb155a3f5e1e6c4b20ecae638db2272b130151718130c14177b7c4f1240304688254e5", "a3e428c0606701bb535cd4c7d979c9ef2d6a475d3e1199079098c56d9cbb2b9b76e33735a0f4e824ed1f92176833c2ae"), // 0020_mailbox_management.sql
    (21, "52ea21d65f03c5532a28ad096cbc4b04c5cb60b133bad28ef6a7e1163feaa3bab53febe90e67be2d00c3f3c97bcd7697", "23d5e1563ea7026de1f669f20feda2da0a36d43386a14bea722324b0a8e69f437a0a3448ed7136d0b96e46891c072d95"), // 0021_inline_image_fidelity.sql
    (22, "664695e019b4c38e196c9190dec819c8b99563427af16b98a09e27c903c0338185618e25df404294518419e97b3a61af", "10f7741be0496aa4229228ea0f54a03f863034ddcc33bfb68932100b419ed6d4a3cf87e50d50c6f62dbad02153fdac28"), // 0022_octet_stream_cid_fidelity.sql
    (23, "e77ef389db8ce13f8c0f905dd3b3ae8079aad392a539345164ec2293f909db019857aa2d4bda8e8526cfcb55a9392d2c", "30f22e1d25f7771e3774b391adbf74f068eeeddeba94091847cb85982c325527ee65c5657f6b30311545bf45d7f13484"), // 0023_bmp_inline_image_fidelity.sql
    (24, "b342cf360a678666ccaebeec1f43f666df4c31259b2a03e9aaff7b5b1b7d2d3dce5bee12bd75d95b46ff6aee74f250a3", "18e60404db831fc92ef35749367a2b578cd0b1d34b56200853a1c257272457dae771aa9132958c3a051fab5ad19f6d8a"), // 0024_full_message_sync_preference.sql
    (25, "26030a659c364f75ad77ac93624ccda5095013599a47b8d389f7e2d0a4892c4933bbe891b27b59f2e46295d8ba9b18dd", "500200c111994b27c5e297730ab2eceb92b51eb5cc5ed3bc95af1f4089cc28c77c195110091e1d37d67daa7b989c8807"), // 0025_contacts.sql
    (26, "685a608a6d3155d31a6e3845adbb774a582e8c9c9f4fc5704d7aebdc2665aa514e02353976b50c3ce206fafbf2d89acf", "5105eb11f88f212da7ebd9f63fb89f95b7e507c139392ce74717b587e438b140ed642cf768c6ef767feab7af30301808"), // 0026_template_recipients.sql
    (27, "a84c6533209da41c09da0c30a4a6dec53898a9822aa02eb10c9f5dda13a44e917d5127cde44468a33a30953673566573", "befed6de0e35f34b1f945f4361cdce56e7716cd1232ece525acd8afc564734d0e11da6fa7acd5c5e63f7314faf41b6e7"), // 0027_selective_imap_sections.sql
    (28, "c8a4135b79018060fd1665bcf2bc0b4ffe347a56ca9591ccbbcff28f86d5834edd10ad32a26f78f0fbbeba053074aa5f", "0db8d63d743a506ffd58022c27dcf4df8e67dbe82d20971f9d0a385f3e8ee6c76a0c47bb846d8e0c60848d695c5444dc"), // 0028_selective_cid_body_refresh.sql
    (29, "e1ec1fe77888f8550c638cd2f25ed69f952564760f08d4dea4f631979e08bdc54d8577e84d7159f270290b1c141d975e", "159680f836e3c3a420f4d9824902e9f6eb8ee5e9642e8534af0d46160829b5421d95190ee7f2ca3ab1fa5bceac47c482"), // 0029_attachment_filename_refresh.sql
];

fn decode_hex(value: &str) -> [u8; 48] {
    fn nibble(byte: u8) -> u8 {
        match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => unreachable!("checksum table only holds lowercase hex"),
        }
    }
    debug_assert_eq!(value.len(), 96);
    let mut decoded = [0u8; 48];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        decoded[index] = (nibble(pair[0]) << 4) | nibble(pair[1]);
    }
    decoded
}

pub async fn repair_crlf_migration_checksums(pool: &SqlitePool) {
    let table_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false);
    if !table_exists {
        return;
    }
    let mut repaired: u64 = 0;
    for (version, crlf_hex, lf_hex) in CRLF_TO_LF_CHECKSUMS {
        let crlf = decode_hex(crlf_hex);
        let lf = decode_hex(lf_hex);
        match sqlx::query(
            "UPDATE _sqlx_migrations SET checksum = ? WHERE version = ? AND checksum = ?",
        )
        .bind(lf.as_slice())
        .bind(version)
        .bind(crlf.as_slice())
        .execute(pool)
        .await
        {
            Ok(result) => repaired += result.rows_affected(),
            Err(error) => {
                tracing::warn!(version, %error, "migration checksum repair skipped for this version");
            }
        }
    }
    if repaired > 0 {
        tracing::warn!(
            repaired,
            "rewrote legacy CRLF migration checksums to LF values"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::repository::MIGRATOR;
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
    use std::str::FromStr;

    async fn open_test_pool(path: &std::path::Path) -> SqlitePool {
        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))
            .unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap()
    }

    #[test]
    fn table_matches_the_embedded_migrator_lf_checksums() {
        // Guards the table against migration edits/renames: the LF column must
        // equal what MIGRATOR embeds at compile time.
        for (version, _, lf_hex) in CRLF_TO_LF_CHECKSUMS {
            let embedded = MIGRATOR
                .iter()
                .find(|migration| migration.version == *version)
                .unwrap_or_else(|| panic!("migration {version} missing from embedded migrator"));
            assert_eq!(
                decode_hex(lf_hex).as_slice(),
                &*embedded.checksum,
                "LF checksum for migration {version} is out of date"
            );
        }
        assert_eq!(MIGRATOR.iter().count(), CRLF_TO_LF_CHECKSUMS.len());
    }

    #[tokio::test]
    async fn rewrites_known_crlf_checksums_and_migrator_validates_afterwards() {
        let directory = tempfile::tempdir().unwrap();
        let pool = open_test_pool(&directory.path().join("content.sqlite")).await;
        MIGRATOR.run(&pool).await.unwrap();

        for (version, crlf_hex, _) in CRLF_TO_LF_CHECKSUMS {
            let crlf = decode_hex(crlf_hex);
            sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?")
                .bind(crlf.as_slice())
                .bind(version)
                .execute(&pool)
                .await
                .unwrap();
        }

        repair_crlf_migration_checksums(&pool).await;

        let stored: Vec<(i64, Vec<u8>)> =
            sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(stored.len(), CRLF_TO_LF_CHECKSUMS.len());
        for ((version, checksum), (expected_version, _, lf_hex)) in
            stored.iter().zip(CRLF_TO_LF_CHECKSUMS)
        {
            assert_eq!(version, expected_version);
            assert_eq!(checksum, &decode_hex(lf_hex).to_vec());
        }
        MIGRATOR.run(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn leaves_lf_checksums_untouched() {
        let directory = tempfile::tempdir().unwrap();
        let pool = open_test_pool(&directory.path().join("content.sqlite")).await;
        MIGRATOR.run(&pool).await.unwrap();
        let before: Vec<(i64, Vec<u8>)> =
            sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&pool)
                .await
                .unwrap();

        repair_crlf_migration_checksums(&pool).await;

        let after: Vec<(i64, Vec<u8>)> =
            sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(before, after);
        MIGRATOR.run(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn skips_fresh_databases_without_the_migrations_table() {
        let directory = tempfile::tempdir().unwrap();
        let pool = open_test_pool(&directory.path().join("content.sqlite")).await;
        repair_crlf_migration_checksums(&pool).await;
    }
}
