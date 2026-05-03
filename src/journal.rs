use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::encoding::{decode_path, encode_path};
use crate::error::{AppError, Result};
use crate::filesystem::{PlannedMove, finish_move_set, rollback_move_set};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalStage {
    Planned,
    OutboundComplete,
    InboundComplete,
    Rollback,
}

impl JournalStage {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "" | "planned" => Ok(Self::Planned),
            "outbound-complete" => Ok(Self::OutboundComplete),
            "inbound-complete" => Ok(Self::InboundComplete),
            "rollback" => Ok(Self::Rollback),
            _ => Err(AppError::message(format!(
                "Swap journal has unknown stage: {value}"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::OutboundComplete => "outbound-complete",
            Self::InboundComplete => "inbound-complete",
            Self::Rollback => "rollback",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Journal {
    pub stage: JournalStage,
    pub from_guid: String,
    pub to_guid: String,
    pub outbound: Vec<PlannedMove>,
    pub inbound: Vec<PlannedMove>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryOutcome {
    NoJournal,
    RolledBackToFrom,
    CompletedToTarget,
}

#[derive(Clone, Debug)]
pub struct JournalStore {
    pub root: PathBuf,
    pub active_file: PathBuf,
    pub journal_file: PathBuf,
}

impl JournalStore {
    pub fn new(root: PathBuf, active_file: PathBuf, journal_file: PathBuf) -> Self {
        Self {
            root,
            active_file,
            journal_file,
        }
    }
}

pub fn write_journal(
    store: &JournalStore,
    mut journal: Journal,
    stage: JournalStage,
) -> Result<()> {
    journal.stage = stage;
    fs::create_dir_all(&store.root)?;
    let mut out = File::create(&store.journal_file)?;
    writeln!(out, "version\t1")?;
    writeln!(out, "stage\t{}", journal.stage.as_str())?;
    writeln!(out, "from\t{}", journal.from_guid)?;
    writeln!(out, "to\t{}", journal.to_guid)?;
    for mv in &journal.outbound {
        writeln!(
            out,
            "out\t{}\t{}",
            encode_path(&mv.from),
            encode_path(&mv.to)
        )?;
    }
    for mv in &journal.inbound {
        writeln!(
            out,
            "in\t{}\t{}",
            encode_path(&mv.from),
            encode_path(&mv.to)
        )?;
    }
    Ok(())
}

pub fn read_journal(path: &Path) -> Result<Option<Journal>> {
    let Ok(file) = File::open(path) else {
        return Ok(None);
    };
    let mut stage = JournalStage::Planned;
    let mut from_guid = String::new();
    let mut to_guid = String::new();
    let mut outbound = Vec::new();
    let mut inbound = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        let parts: Vec<_> = line.split('\t').collect();
        match parts.as_slice() {
            ["stage", value] => stage = JournalStage::parse(value)?,
            ["from", value] => from_guid = (*value).to_string(),
            ["to", value] => to_guid = (*value).to_string(),
            ["out", from, to] => outbound.push(PlannedMove {
                from: decode_path(from),
                to: decode_path(to),
            }),
            ["in", from, to] => inbound.push(PlannedMove {
                from: decode_path(from),
                to: decode_path(to),
            }),
            _ => {}
        }
    }
    if from_guid.is_empty() || to_guid.is_empty() {
        return Err(AppError::message("Swap journal is malformed"));
    }
    Ok(Some(Journal {
        stage,
        from_guid,
        to_guid,
        outbound,
        inbound,
    }))
}

pub fn clear_journal(path: &Path) {
    let _ = fs::remove_file(path);
}

pub fn recover_journal_core(store: &JournalStore) -> Result<RecoveryOutcome> {
    let Some(journal) = read_journal(&store.journal_file)? else {
        return Ok(RecoveryOutcome::NoJournal);
    };
    let outcome = match journal.stage {
        JournalStage::Planned => {
            rollback_move_set(&journal.outbound)?;
            set_active_desktop(store, &journal.from_guid)?;
            RecoveryOutcome::RolledBackToFrom
        }
        JournalStage::OutboundComplete | JournalStage::InboundComplete => {
            finish_move_set(&journal.outbound)?;
            finish_move_set(&journal.inbound)?;
            set_active_desktop(store, &journal.to_guid)?;
            RecoveryOutcome::CompletedToTarget
        }
        JournalStage::Rollback => {
            rollback_move_set(&journal.inbound)?;
            rollback_move_set(&journal.outbound)?;
            set_active_desktop(store, &journal.from_guid)?;
            RecoveryOutcome::RolledBackToFrom
        }
    };
    clear_journal(&store.journal_file);
    Ok(outcome)
}

fn set_active_desktop(store: &JournalStore, guid: &str) -> Result<()> {
    if let Some(parent) = store.active_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&store.active_file, format!("{guid}\n"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(name: &str) -> JournalStore {
        let root =
            std::env::temp_dir().join(format!("deskicons-journal-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        JournalStore::new(
            root.clone(),
            root.join("active-desktop.txt"),
            root.join("swap.journal"),
        )
    }

    fn journal(stage: JournalStage, root: &Path) -> Journal {
        Journal {
            stage,
            from_guid: "from-guid".to_string(),
            to_guid: "to-guid".to_string(),
            outbound: vec![PlannedMove {
                from: root.join("desktop").join("a.txt"),
                to: root
                    .join("sets")
                    .join("from-guid")
                    .join("files")
                    .join("a.txt"),
            }],
            inbound: vec![PlannedMove {
                from: root
                    .join("sets")
                    .join("to-guid")
                    .join("files")
                    .join("b.txt"),
                to: root.join("desktop").join("b.txt"),
            }],
        }
    }

    #[test]
    fn journal_round_trips_unicode_paths_and_typed_stage() {
        let store = store("roundtrip");
        let mut journal = journal(JournalStage::InboundComplete, &store.root);
        journal.outbound[0].from = store.root.join("ä-東京-😀 %.txt");

        write_journal(&store, journal.clone(), JournalStage::OutboundComplete).unwrap();

        let actual = read_journal(&store.journal_file).unwrap().unwrap();
        assert_eq!(actual.stage, JournalStage::OutboundComplete);
        assert_eq!(actual.outbound[0].from, store.root.join("ä-東京-😀 %.txt"));
        let _ = fs::remove_dir_all(store.root);
    }

    #[test]
    fn recover_planned_rolls_back_outbound_and_clears_journal() {
        let store = store("planned");
        let journal = journal(JournalStage::Planned, &store.root);
        fs::create_dir_all(journal.outbound[0].to.parent().unwrap()).unwrap();
        fs::write(&journal.outbound[0].to, "parked").unwrap();
        write_journal(&store, journal.clone(), JournalStage::Planned).unwrap();

        let outcome = recover_journal_core(&store).unwrap();

        assert_eq!(outcome, RecoveryOutcome::RolledBackToFrom);
        assert_eq!(
            fs::read_to_string(&journal.outbound[0].from).unwrap(),
            "parked"
        );
        assert_eq!(
            fs::read_to_string(&store.active_file).unwrap(),
            "from-guid\n"
        );
        assert!(!store.journal_file.exists());
        let _ = fs::remove_dir_all(store.root);
    }

    #[test]
    fn recover_outbound_complete_finishes_both_move_sets() {
        let store = store("outbound");
        let journal = journal(JournalStage::OutboundComplete, &store.root);
        fs::create_dir_all(journal.outbound[0].to.parent().unwrap()).unwrap();
        fs::create_dir_all(journal.inbound[0].from.parent().unwrap()).unwrap();
        fs::write(&journal.outbound[0].to, "parked").unwrap();
        fs::write(&journal.inbound[0].from, "incoming").unwrap();
        write_journal(&store, journal.clone(), JournalStage::OutboundComplete).unwrap();

        let outcome = recover_journal_core(&store).unwrap();

        assert_eq!(outcome, RecoveryOutcome::CompletedToTarget);
        assert_eq!(
            fs::read_to_string(&journal.outbound[0].to).unwrap(),
            "parked"
        );
        assert_eq!(
            fs::read_to_string(&journal.inbound[0].to).unwrap(),
            "incoming"
        );
        assert_eq!(fs::read_to_string(&store.active_file).unwrap(), "to-guid\n");
        assert!(!store.journal_file.exists());
        let _ = fs::remove_dir_all(store.root);
    }
}
