use eve_wrench_core::{SettingsEntry, SettingsKind};

#[derive(Clone, Default, PartialEq)]
pub(super) struct SearchQuery {
    terms: Vec<String>,
}

impl SearchQuery {
    pub(super) fn new(value: &str) -> Self {
        Self {
            terms: value.split_whitespace().map(str::to_lowercase).collect(),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    pub(super) fn matches(&self, entry: &SettingsEntry, kind: &str, modified: &str) -> bool {
        if self.is_empty() {
            return true;
        }

        let fields: Vec<String> = [
            Some(entry.id.as_str()),
            Some(entry.display_name.as_str()),
            entry.alias.as_deref(),
            entry.character.as_ref().map(|c| c.name.as_str()),
            entry
                .character
                .as_ref()
                .and_then(|c| c.corporation.as_deref()),
            entry.character.as_ref().and_then(|c| c.alliance.as_deref()),
            entry
                .character
                .as_ref()
                .and_then(|c| c.created_at.as_deref()),
            Some(entry.profile.as_str()),
            Some(entry.path.as_str()),
            Some(entry.server.id()),
            Some(entry.server.display_name()),
            Some(entry.server.short_name()),
            Some(match entry.kind {
                SettingsKind::User => "account user",
                SettingsKind::Char => "character char",
            }),
            Some(kind),
            Some(modified),
        ]
        .into_iter()
        .flatten()
        .map(str::to_lowercase)
        .collect();

        self.terms
            .iter()
            .all(|term| fields.iter().any(|field| field.contains(term)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eve_wrench_core::{CharacterDetails, Server};

    fn entry(kind: SettingsKind, id: &str) -> SettingsEntry {
        SettingsEntry {
            path: format!("C:/EVE/settings_Default/core_{id}.dat"),
            id: id.into(),
            kind,
            server: Server::Tranquility,
            profile: "Default".into(),
            display_name: id.into(),
            character: None,
            alias: None,
            modified_time: 0,
        }
    }

    #[test]
    fn accounts_match_aliases_and_ids_and_exclude_unrelated_rows() {
        let mut account = entry(SettingsKind::User, "12345678");
        account.alias = Some("Sukinin".into());
        // The alias is searchable even before the display name is refreshed.
        assert!(SearchQuery::new("  SUKININ  ").matches(&account, "Accounts", "2d ago"));
        assert!(SearchQuery::new("345678").matches(&account, "Accounts", "2d ago"));
        assert!(!SearchQuery::new("Sukinin").matches(
            &entry(SettingsKind::User, "87654321"),
            "Accounts",
            "2d ago",
        ));
    }

    #[test]
    fn character_names_corporations_and_aliases_remain_searchable() {
        let mut character = entry(SettingsKind::Char, "90000001");
        character.character = Some(CharacterDetails {
            name: "Sukinin Syn".into(),
            corporation: Some("Exploration Fleet".into()),
            alliance: Some("Deep Space Alliance".into()),
            created_at: Some("2008-05-06T07:08:09Z".into()),
            portrait: None,
        });
        character.alias = Some("Scout".into());
        for query in [
            "Sukinin Syn",
            "exploration",
            "SCOUT",
            "90000001",
            "deep space",
            "2008-05-06",
        ] {
            assert!(SearchQuery::new(query).matches(&character, "Characters", "1h ago"));
        }
        assert!(!SearchQuery::new("other pilot").matches(&character, "Characters", "1h ago"));
    }

    #[test]
    fn searches_metadata_and_requires_every_word_to_match() {
        let mut account = entry(SettingsKind::User, "12345678");
        account.alias = Some("Sukinin".into());
        for query in [
            "Sukinin 12345678",
            "Tranquility Default",
            "TQ account",
            "settings_Default/core_12345678.dat",
            "2d ago",
            "账户",
        ] {
            assert!(SearchQuery::new(query).matches(&account, "账户", "2d ago"));
        }
        assert!(!SearchQuery::new("Sukinin missing").matches(&account, "Accounts", "2d ago"));
    }

    #[test]
    fn empty_search_includes_both_kinds_and_clearing_restores_rows() {
        for kind in [SettingsKind::User, SettingsKind::Char] {
            let entry = entry(kind, "12345678");
            assert!(!SearchQuery::new("missing").matches(&entry, "", ""));
            assert!(SearchQuery::new(" \t\n ").matches(&entry, "", ""));
            assert!(SearchQuery::default().matches(&entry, "", ""));
        }
    }
}
