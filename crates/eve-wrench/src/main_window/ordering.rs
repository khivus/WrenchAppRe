use super::SortColumn;
use eve_wrench_core::SettingsEntry;
use eve_wrench_core::SettingsKind;
use std::cmp::Ordering;

pub(super) fn compare_entries(
    a: &SettingsEntry,
    b: &SettingsEntry,
    column: SortColumn,
) -> Ordering {
    let name = |entry: &SettingsEntry| match entry.kind {
        SettingsKind::User => entry.alias.clone().unwrap_or_default(),
        SettingsKind::Char => entry.display_name.clone(),
    };
    let character_text = |entry: &SettingsEntry| {
        entry
            .character
            .as_ref()
            .and_then(|info| match column {
                SortColumn::Corporation => info.corporation.clone(),
                SortColumn::Alliance => info.alliance.clone(),
                SortColumn::CreationDate => info.created_at.clone(),
                _ => None,
            })
            .unwrap_or_default()
    };
    match column {
        SortColumn::Id => compare_entry_ids(a, b),
        SortColumn::Name => name(a).to_lowercase().cmp(&name(b).to_lowercase()),
        SortColumn::Time => a.modified_time.cmp(&b.modified_time),
        _ => character_text(a)
            .to_lowercase()
            .cmp(&character_text(b).to_lowercase()),
    }
    .then_with(|| compare_entry_ids(a, b))
}

pub(super) fn compare_entry_ids(a: &SettingsEntry, b: &SettingsEntry) -> Ordering {
    match (a.id.parse::<u64>(), b.id.parse::<u64>()) {
        (Ok(a), Ok(b)) => a.cmp(&b),
        _ => a.id.cmp(&b.id),
    }
    .then_with(|| a.path.cmp(&b.path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eve_wrench_core::{Server, SettingsKind};

    #[test]
    fn sorts_numeric_ids_independently_of_names_and_aliases() {
        for kind in [SettingsKind::Char, SettingsKind::User] {
            let mut entries: Vec<_> = ["100", "2", "90"]
                .into_iter()
                .map(|id| SettingsEntry {
                    id: id.into(),
                    path: id.into(),
                    kind,
                    server: Server::Tranquility,
                    profile: "Default".into(),
                    display_name: "Renamed".into(),
                    alias: Some("Custom alias".into()),
                    character: None,
                    modified_time: 0,
                })
                .collect();
            entries.sort_by(compare_entry_ids);
            assert_eq!(
                entries
                    .iter()
                    .map(|entry| entry.id.as_str())
                    .collect::<Vec<_>>(),
                ["2", "90", "100"]
            );
        }
    }

    #[test]
    fn account_ids_and_custom_names_sort_separately() {
        let mut entries: Vec<_> = [("2", Some("Zulu")), ("10", Some("alpha")), ("100", None)]
            .into_iter()
            .map(|(id, alias)| SettingsEntry {
                id: id.into(),
                path: id.into(),
                kind: SettingsKind::User,
                server: Server::Tranquility,
                profile: "Default".into(),
                display_name: alias.unwrap_or(id).into(),
                alias: alias.map(str::to_string),
                character: None,
                modified_time: 0,
            })
            .collect();
        entries.sort_by(|a, b| compare_entries(a, b, SortColumn::Name));
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            ["100", "10", "2"]
        );
        entries.sort_by(|a, b| compare_entries(a, b, SortColumn::Id));
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            ["2", "10", "100"]
        );
    }
}
