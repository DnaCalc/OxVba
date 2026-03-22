//! HAL journal model for recording and replaying host interactions.
//!
//! Uses JSON (serde_json) matching OxReplay's format decision — human-readable
//! for audit/debug, language-agnostic.

use serde::{Deserialize, Serialize};

/// Top-level journal structure containing all recorded HAL interactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HalJournal {
    pub schema: String,
    pub profile: String,
    pub entries: Vec<HalJournalEntry>,
}

impl HalJournal {
    pub fn new(profile: &str) -> Self {
        Self {
            schema: "oxvba.hal-journal.v1".to_string(),
            profile: profile.to_string(),
            entries: Vec::new(),
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// A single recorded HAL interaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HalJournalEntry {
    pub sequence: u64,
    pub capability: String,
    pub operation: String,
    /// OxReplay-compatible event label.
    pub source_label: String,
    /// OxReplay cross-lane family for normalization.
    pub normalized_family: String,
    pub result: serde_json::Value,
}

impl HalJournalEntry {
    pub fn new(
        sequence: u64,
        capability: &str,
        operation: &str,
        family: &str,
        result: serde_json::Value,
    ) -> Self {
        Self {
            sequence,
            capability: capability.to_string(),
            operation: operation.to_string(),
            source_label: format!("hal.{capability}.{operation}"),
            normalized_family: family.to_string(),
            result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journal_roundtrip_json() {
        let mut journal = HalJournal::new("windows");
        journal.entries.push(HalJournalEntry::new(
            1,
            "time",
            "timer_ticks",
            "time.timer",
            serde_json::json!(42),
        ));
        journal.entries.push(HalJournalEntry::new(
            2,
            "ui",
            "msg_box",
            "ui.msgbox",
            serde_json::json!(1),
        ));

        let json = journal.to_json().expect("serialize");
        let restored = HalJournal::from_json(&json).expect("deserialize");

        assert_eq!(restored.schema, "oxvba.hal-journal.v1");
        assert_eq!(restored.profile, "windows");
        assert_eq!(restored.entries.len(), 2);
        assert_eq!(restored.entries[0].sequence, 1);
        assert_eq!(restored.entries[0].capability, "time");
        assert_eq!(restored.entries[0].normalized_family, "time.timer");
        assert_eq!(restored.entries[1].operation, "msg_box");
    }
}
