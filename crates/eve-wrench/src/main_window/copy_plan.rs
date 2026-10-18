use super::Source;
use eve_wrench_core::SettingsEntry;

#[derive(Clone)]
pub(super) struct CopyPlan {
    pub source: Source,
    pub targets: Vec<String>,
}

// Reject the whole selection if any target has no matching source or is itself
// a source. Plans are frozen before confirmation, so its description and writes agree.
pub(super) fn plan_copy(sources: &[Source], targets: &[SettingsEntry]) -> Option<Vec<CopyPlan>> {
    if !can_copy(sources, targets) {
        return None;
    }
    Some(
        sources
            .iter()
            .filter_map(|source| {
                let targets: Vec<_> = targets
                    .iter()
                    .filter(|target| target.kind == source.kind())
                    .map(|target| target.path.clone())
                    .collect();
                (!targets.is_empty()).then(|| CopyPlan {
                    source: source.clone(),
                    targets,
                })
            })
            .collect(),
    )
}

// Rendering only needs eligibility; allocate the frozen path lists when the
// user actually starts a copy, rather than on every mouse move or scroll frame.
pub(super) fn can_copy(sources: &[Source], targets: &[SettingsEntry]) -> bool {
    !targets.is_empty()
        && targets.iter().all(|target| {
            sources
                .iter()
                .any(|source| source.kind() == target.kind && source.path() != target.path)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use eve_wrench_core::{BackupEntry, Server, SettingsKind};

    fn entry(kind: SettingsKind, path: &str) -> SettingsEntry {
        SettingsEntry {
            path: path.into(),
            id: path.into(),
            kind,
            server: Server::Tranquility,
            profile: "Default".into(),
            display_name: path.into(),
            character: None,
            alias: None,
            modified_time: 0,
        }
    }

    #[test]
    fn paired_defaults_route_each_target_to_the_matching_source() {
        let sources = vec![
            Source::Entry(entry(SettingsKind::Char, "source-char")),
            Source::Entry(entry(SettingsKind::User, "source-account")),
        ];
        let mut targets = vec![
            entry(SettingsKind::User, "target-account"),
            entry(SettingsKind::Char, "target-char"),
        ];
        let plans = plan_copy(&sources, &targets).unwrap();
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].source.path(), "source-char");
        assert_eq!(plans[0].targets, ["target-char"]);
        assert_eq!(plans[1].source.path(), "source-account");
        assert_eq!(plans[1].targets, ["target-account"]);
        targets.clear();
        assert_eq!(plans[0].targets, ["target-char"]);
    }

    #[test]
    fn rejects_missing_source_or_copying_a_source_onto_itself() {
        let character = entry(SettingsKind::Char, "source-char");
        let sources = [Source::Entry(character.clone())];
        assert!(plan_copy(&sources, &[character]).is_none());
        assert!(plan_copy(&sources, &[entry(SettingsKind::User, "target-account")]).is_none());
        assert!(plan_copy(&sources, &[]).is_none());
        assert!(plan_copy(&[], &[entry(SettingsKind::Char, "target-char")]).is_none());
    }

    #[test]
    fn manual_backups_keep_their_original_single_kind_copy_behavior() {
        let source = Source::Backup(BackupEntry {
            id: "backup".into(),
            name: "Backup".into(),
            path: "backup.bak".into(),
            timestamp: 0,
            kind: SettingsKind::User,
            original_id: "123".into(),
            original_name: None,
        });
        let plans = plan_copy(&[source], &[entry(SettingsKind::User, "target-account")]).unwrap();
        assert_eq!(plans[0].source.path(), "backup.bak");
        assert_eq!(plans[0].targets, ["target-account"]);
    }
}
