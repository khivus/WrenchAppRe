use std::collections::HashSet;
use std::ops::Deref;

use eve_wrench_core::SettingsEntry;

// Keep display order and a path index together so row highlighting and bulk
// selection do not scan every selected entry on each repaint.
#[derive(Default)]
pub(super) struct Targets {
    entries: Vec<SettingsEntry>,
    paths: HashSet<String>,
}

impl Targets {
    pub fn contains_path(&self, path: &str) -> bool {
        self.paths.contains(path)
    }

    pub fn push(&mut self, entry: SettingsEntry) {
        if self.paths.insert(entry.path.clone()) {
            self.entries.push(entry);
        }
    }

    pub fn retain(&mut self, keep: impl FnMut(&SettingsEntry) -> bool) {
        self.entries.retain(keep);
        self.paths = self
            .entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect();
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.paths.clear();
    }
}

impl Deref for Targets {
    type Target = [SettingsEntry];

    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

impl Extend<SettingsEntry> for Targets {
    fn extend<T: IntoIterator<Item = SettingsEntry>>(&mut self, entries: T) {
        for entry in entries {
            self.push(entry);
        }
    }
}

impl FromIterator<SettingsEntry> for Targets {
    fn from_iter<T: IntoIterator<Item = SettingsEntry>>(entries: T) -> Self {
        let mut targets = Self::default();
        targets.extend(entries);
        targets
    }
}

#[cfg(test)]
mod tests {
    use super::super::profiles::tests::sample_data;
    use super::Targets;

    #[test]
    fn path_index_stays_in_sync_when_targets_are_added_removed_and_refreshed() {
        let data = sample_data();
        let entries: Vec<_> = data.entries().cloned().collect();
        let mut targets: Targets = entries.iter().cloned().collect();
        targets.extend(entries.iter().cloned());
        assert_eq!(targets.len(), entries.len());
        for entry in &entries {
            assert!(targets.contains_path(&entry.path));
        }
        targets.retain(|entry| entry.id == "2");
        for entry in &entries {
            assert_eq!(targets.contains_path(&entry.path), entry.id == "2");
        }
        targets = entries.iter().cloned().collect();
        assert_eq!(targets.len(), entries.len());
        targets.clear();
        assert!(targets.is_empty());
        for entry in &entries {
            assert!(!targets.contains_path(&entry.path));
        }
    }
}
