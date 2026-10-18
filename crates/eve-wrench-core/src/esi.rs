use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use serde::Deserialize;

use crate::model::CharacterDetails;

const ESI_BASE: &str = "https://esi.evetech.net/latest";

pub(crate) static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
    ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .user_agent(concat!("EVE-Wrench/", env!("CARGO_PKG_VERSION")))
            .timeout_global(Some(Duration::from_secs(15)))
            .build(),
    )
});

static CHAR_CACHE: LazyLock<Mutex<HashMap<i64, CharacterDetails>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static CORP_CACHE: LazyLock<Mutex<HashMap<i64, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static ALLIANCE_CACHE: LazyLock<Mutex<HashMap<i64, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Deserialize)]
struct EsiCharacter {
    name: String,
    corporation_id: i64,
    alliance_id: Option<i64>,
    birthday: Option<String>,
}

#[derive(Deserialize)]
struct EsiCorporation {
    name: String,
}

#[derive(Deserialize)]
struct EsiAlliance {
    name: String,
}

fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, String> {
    AGENT
        .get(url)
        .call()
        .map_err(|e| format!("ESI request failed: {}", e))?
        .body_mut()
        .read_json()
        .map_err(|e| format!("Failed to parse ESI response: {}", e))
}

fn corporation_name(corporation_id: i64) -> Option<String> {
    if let Some(name) = CORP_CACHE.lock().ok()?.get(&corporation_id) {
        return Some(name.clone());
    }
    let corp: EsiCorporation =
        get_json(&format!("{}/corporations/{}/", ESI_BASE, corporation_id)).ok()?;
    CORP_CACHE
        .lock()
        .ok()?
        .insert(corporation_id, corp.name.clone());
    Some(corp.name)
}

fn alliance_name(alliance_id: i64) -> Option<String> {
    if let Some(name) = ALLIANCE_CACHE.lock().ok()?.get(&alliance_id) {
        return Some(name.clone());
    }
    let alliance: EsiAlliance = get_json(&format!("{ESI_BASE}/alliances/{alliance_id}/")).ok()?;
    ALLIANCE_CACHE
        .lock()
        .ok()?
        .insert(alliance_id, alliance.name.clone());
    Some(alliance.name)
}

pub fn character_details(character_id: i64) -> Result<CharacterDetails, String> {
    if let Some(cached) = CHAR_CACHE
        .lock()
        .ok()
        .and_then(|cache| cache.get(&character_id).cloned())
    {
        return Ok(cached);
    }

    let character: EsiCharacter = get_json(&format!("{}/characters/{}/", ESI_BASE, character_id))?;
    let details = CharacterDetails {
        name: character.name,
        corporation: corporation_name(character.corporation_id),
        alliance: character.alliance_id.and_then(alliance_name),
        created_at: character.birthday,
        portrait: portrait(character_id),
    };

    if let Ok(mut cache) = CHAR_CACHE.lock() {
        cache.insert(character_id, details.clone());
    }
    Ok(details)
}

fn portrait(character_id: i64) -> Option<Arc<[u8]>> {
    let url = format!(
        "https://images.evetech.net/characters/{}/portrait?size=64",
        character_id
    );
    let bytes = AGENT.get(&url).call().ok()?.body_mut().read_to_vec().ok()?;
    Some(bytes.into())
}

// Looks up several characters concurrently; failed lookups are left out so
// those entries keep showing their raw ID.
pub fn fetch_character_details(ids: &[i64]) -> HashMap<i64, CharacterDetails> {
    const WORKERS: usize = 8;
    let chunk_size = ids.len().div_ceil(WORKERS).max(1);

    std::thread::scope(|scope| {
        let handles: Vec<_> = ids
            .chunks(chunk_size)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .filter_map(|id| character_details(*id).ok().map(|d| (*id, d)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .flatten()
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::EsiCharacter;

    #[test]
    fn public_character_fields_allow_missing_alliance_and_birthday() {
        let with_alliance: EsiCharacter = serde_json::from_str(
            r#"{"name":"Pilot","corporation_id":12,"alliance_id":34,"birthday":"2008-05-06T07:08:09Z"}"#,
        ).unwrap();
        assert_eq!(with_alliance.alliance_id, Some(34));
        assert_eq!(
            with_alliance.birthday.as_deref(),
            Some("2008-05-06T07:08:09Z")
        );
        let without: EsiCharacter =
            serde_json::from_str(r#"{"name":"Pilot","corporation_id":12}"#).unwrap();
        assert!(without.alliance_id.is_none());
        assert!(without.birthday.is_none());
    }
}
