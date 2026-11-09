use super::*;
use ge_core::mission::loader::work::{Budget, WorkPoll};

fn origin(hash: u32) -> Origin {
    Origin {
        index: "File_USA.bin",
        hash,
        version: 10,
        bytes: 44,
        archive: "authored.000".into(),
        offset: 0,
    }
}
fn metadata() -> Metadata {
    Metadata {
        origin: origin(METADATA),
        bytes: Arc::new(vec![0; 44]),
        digest: [1; 32],
        header: sound_info::Header {
            resource_hash: METADATA,
            version: 10,
            file_bytes: 44,
            reserved_raw: [0; 12],
            minimum_hash: 0,
            maximum_hash: 0,
            record_count: 0,
        },
    }
}
fn command(id: u64) -> Command {
    Command {
        id,
        token: 0x2d50_0079,
        flags: 0,
        sequence: id as u32,
    }
}
fn bank(digest: u8) -> ResidentBank {
    ResidentBank {
        origin: origin(0x2d50_0079),
        bytes: Arc::new(vec![digest; 64]),
        sounds: 0,
        waves: 0,
        digest: [digest; 32],
    }
}
fn fields() -> outer::BankFields {
    outer::BankFields {
        word_4c: 0x2d50_0079,
        word_50: 0,
        word_54: 0,
        byte_58: 1,
    }
}

#[test]
fn bank_acknowledgement_does_not_substitute_for_shared_metadata() {
    let mut service = Service::new();
    service.pending.insert(1, command(1));
    register(&mut service, Loaded::Bank(command(1), bank(2))).unwrap();
    assert!(service.pending.is_empty());
    assert!(!service.complete(fields()));
    register(&mut service, Loaded::SoundInfo(metadata())).unwrap();
    assert!(service.complete(fields()));
    let mut missing = fields();
    missing.word_50 = 0x2d50_0014;
    assert!(!service.complete(missing));
}

#[test]
fn cancellation_recovery_owns_metadata_before_unfinished_bank_callbacks() {
    let first = AttemptId::new(1).unwrap();
    let second = AttemptId::new(2).unwrap();
    let mut old = WorkOwner::new(first, Budget { tickets: 8 }).unwrap();
    old.enqueue(first, 0x2d50_0079).unwrap();
    old.cancel(first).unwrap();
    let mut service = Service::new();
    service.pending.insert(1, command(1));
    let mut work = WorkOwner::new(second, Budget { tickets: 8 }).unwrap();
    let mut batch = Batch::new();
    batch
        .recover(&mut service, &mut work, second, Some(&origin(METADATA)))
        .unwrap();
    assert!(matches!(
        batch.tasks[&work.tickets()[0].ticket.id()],
        Task::SoundInfo(_)
    ));
    assert!(matches!(
        batch.tasks[&work.tickets()[1].ticket.id()],
        Task::Bank(_)
    ));
    assert_eq!(work.poll(second).unwrap(), WorkPoll::Busy);
    assert_eq!(service.pending.len(), 1);
    assert!(service.metadata.is_none());
}

#[test]
fn changed_cached_payload_leaves_backend_command_unacknowledged() {
    let mut service = Service::new();
    register(&mut service, Loaded::SoundInfo(metadata())).unwrap();
    register(&mut service, Loaded::Bank(command(1), bank(2))).unwrap();
    service.pending.insert(2, command(2));
    assert!(register(&mut service, Loaded::Bank(command(2), bank(3))).is_err());
    assert!(service.pending.contains_key(&2));
    assert_eq!(service.banks[&0x2d50_0079].digest, [2; 32]);
    assert!(!service.complete(fields()));
}
