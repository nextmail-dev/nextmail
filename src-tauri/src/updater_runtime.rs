use std::time::Duration;

use serde::Deserialize;
use tauri::{AppHandle, Url};
use tauri_plugin_updater::UpdaterExt;

use crate::core::{CommandError, CommandResult, UpdateCheckResult};

const GEO_ENDPOINT: &str = "https://api.next-mail.app/api/v1/geo";
const DIRECT_UPDATE_ENDPOINT: &str =
    "https://github.com/nextmail-dev/nextmail/releases/latest/download/latest.json";
const CN_UPDATE_ENDPOINT: &str = "https://proxy.next-mail.app/https://github.com/nextmail-dev/nextmail/releases/latest/download/latest-cn.json";

#[derive(Deserialize)]
struct GeoResponse {
    country_code: String,
}

pub async fn check(app: &AppHandle) -> CommandResult<UpdateCheckResult> {
    let (public_key, endpoints) = updater_configuration().await?;
    let updater = app
        .updater_builder()
        .pubkey(public_key)
        .endpoints(endpoints)
        .map_err(|_| CommandError::new("update.not_configured"))?
        .build()
        .map_err(|_| CommandError::new("update.not_configured"))?;
    let update = updater
        .check()
        .await
        .map_err(|_| CommandError::retryable("update.check_failed"))?;
    let current_version = env!("CARGO_PKG_VERSION").to_owned();
    Ok(match update {
        Some(update) => UpdateCheckResult {
            available: true,
            current_version,
            version: Some(update.version),
            notes: update.body,
        },
        None => UpdateCheckResult {
            available: false,
            current_version,
            version: None,
            notes: None,
        },
    })
}

pub async fn install(app: &AppHandle) -> CommandResult<()> {
    let (public_key, endpoints) = updater_configuration().await?;
    let updater = app
        .updater_builder()
        .pubkey(public_key)
        .endpoints(endpoints)
        .map_err(|_| CommandError::new("update.not_configured"))?
        .build()
        .map_err(|_| CommandError::new("update.not_configured"))?;
    let update = updater
        .check()
        .await
        .map_err(|_| CommandError::retryable("update.check_failed"))?
        .ok_or_else(|| CommandError::new("update.not_available"))?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|_| CommandError::retryable("update.install_failed"))?;
    app.restart()
}

async fn updater_configuration() -> CommandResult<(&'static str, Vec<Url>)> {
    let public_key = option_env!("NEXTMAIL_UPDATER_PUBLIC_KEY")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CommandError::new("update.not_configured"))?;
    let endpoints = update_endpoints(is_mainland_china().await)
        .into_iter()
        .map(Url::parse)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| CommandError::new("update.not_configured"))?;
    Ok((public_key, endpoints))
}

async fn is_mainland_china() -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .user_agent(concat!("NextMail/", env!("CARGO_PKG_VERSION")))
        .build()
    else {
        return false;
    };
    let Ok(response) = client.get(GEO_ENDPOINT).send().await else {
        return false;
    };
    if !response.status().is_success() {
        return false;
    }
    let Ok(body) = response.bytes().await else {
        return false;
    };
    geo_response_is_mainland_china(&body)
}

fn geo_response_is_mainland_china(body: &[u8]) -> bool {
    serde_json::from_slice::<GeoResponse>(body)
        .is_ok_and(|response| response.country_code.eq_ignore_ascii_case("CN"))
}

fn update_endpoints(mainland_china: bool) -> [&'static str; 2] {
    if mainland_china {
        [CN_UPDATE_ENDPOINT, DIRECT_UPDATE_ENDPOINT]
    } else {
        [DIRECT_UPDATE_ENDPOINT, CN_UPDATE_ENDPOINT]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regional_endpoint_order_keeps_the_other_transport_as_backup() {
        assert_eq!(
            update_endpoints(true),
            [CN_UPDATE_ENDPOINT, DIRECT_UPDATE_ENDPOINT]
        );
        assert_eq!(
            update_endpoints(false),
            [DIRECT_UPDATE_ENDPOINT, CN_UPDATE_ENDPOINT]
        );
        assert!(CN_UPDATE_ENDPOINT.starts_with("https://proxy.next-mail.app/https://github.com/"));
    }

    #[test]
    fn geo_response_requires_explicit_cn_country_code() {
        assert!(geo_response_is_mainland_china(
            br#"{"ip":"a.b.c.d","type":"ipv4","country_code":"CN"}"#
        ));
        assert!(geo_response_is_mainland_china(br#"{"country_code":"cn"}"#));
        assert!(!geo_response_is_mainland_china(br#"{"country_code":"US"}"#));
        assert!(!geo_response_is_mainland_china(br#"{"country":"CN"}"#));
        assert!(!geo_response_is_mainland_china(b"not-json"));
    }
}
