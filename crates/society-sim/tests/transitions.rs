//! Regression coverage for authoritative transitions and snapshot validation.

use society_domain::{
    Action, ActionProposal, Position, RejectionReason, ResidentId, ResourceKind, StructureKind,
    WorldEvent,
};
use society_sim::{SnapshotError, Village, VillageSnapshot, WorldObjectKind};

fn propose(village: &mut Village, action: Action) -> WorldEvent {
    village.apply(ActionProposal {
        resident: ResidentId(1),
        expected_version: village.version(),
        idempotency_key: format!("test-{}", village.version().0),
        action,
    })
}

#[test]
fn rejected_construction_preserves_world_and_resident_state() {
    let mut village = Village::new();
    let before = village.snapshot();
    let event = propose(
        &mut village,
        Action::Build {
            structure: StructureKind::LeanTo,
            position: before.residents[0].position,
        },
    );
    assert!(matches!(
        event,
        WorldEvent::ActionRejected {
            reason: RejectionReason::InvalidAction,
            ..
        }
    ));
    assert_eq!(village.version(), before.clock.version());
    assert_eq!(village.residents(), before.residents);
    assert_eq!(village.world_objects(), before.world_objects);
    assert_eq!(village.conversations(), before.conversations);
}

#[test]
fn gathering_pays_for_construction_and_commits_each_action_once() {
    let mut snapshot = Village::new().snapshot();
    let tree = snapshot
        .world_objects
        .iter()
        .find(|object| object.kind == WorldObjectKind::Tree)
        .unwrap()
        .clone();
    snapshot.residents[0].position = tree.position;
    snapshot.residents[0]
        .inventory
        .insert(ResourceKind::Fiber, 2);
    let mut village = Village::from_snapshot(snapshot).unwrap();
    let gather = ActionProposal {
        resident: ResidentId(1),
        expected_version: village.version(),
        idempotency_key: "gather-once".to_owned(),
        action: Action::Gather {
            target: tree.id,
            amount: 5,
        },
    };
    assert!(matches!(
        village.apply(gather.clone()),
        WorldEvent::ActionCommitted { .. }
    ));
    let mut duplicate = gather;
    duplicate.expected_version = village.version();
    assert!(matches!(
        village.apply(duplicate),
        WorldEvent::ActionRejected {
            reason: RejectionReason::DuplicateAction,
            ..
        }
    ));
    assert_eq!(village.residents()[0].inventory[&ResourceKind::Wood], 5);
    let position = tree.position;
    assert!(matches!(
        propose(
            &mut village,
            Action::Build {
                structure: StructureKind::LeanTo,
                position,
            }
        ),
        WorldEvent::ActionCommitted { .. }
    ));
    assert!(
        !village.residents()[0]
            .inventory
            .contains_key(&ResourceKind::Wood)
    );
    assert!(
        !village.residents()[0]
            .inventory
            .contains_key(&ResourceKind::Fiber)
    );
    assert!(village.world_objects().iter().any(|object| {
        object.position == position
            && object.kind == WorldObjectKind::Structure(StructureKind::LeanTo)
    }));
    assert_eq!(village.version().0, 2);
}

#[test]
fn personal_notes_survive_snapshot_restore_and_can_be_revised_and_deleted() {
    let mut village = Village::new();
    assert!(matches!(
        propose(
            &mut village,
            Action::KeepNote {
                text: "  Find water  ".to_owned()
            }
        ),
        WorldEvent::ActionCommitted { .. }
    ));
    let mut village = Village::from_snapshot(village.snapshot()).unwrap();
    let note = village.residents()[0].notes[0].id;
    assert_eq!(village.residents()[0].notes[0].text, "Find water");
    assert!(matches!(
        propose(
            &mut village,
            Action::UpdateNote {
                note,
                text: "  Share water  ".to_owned()
            }
        ),
        WorldEvent::ActionCommitted { .. }
    ));
    assert_eq!(village.residents()[0].notes[0].text, "Share water");
    assert!(matches!(
        propose(&mut village, Action::DeleteNote { note }),
        WorldEvent::ActionCommitted { .. }
    ));
    assert_eq!(
        village.residents()[0].notes,
        Vec::<society_sim::PersonalNote>::new()
    );
}

#[test]
fn snapshot_import_accepts_supported_formats_and_rejects_invalid_state() {
    let original = Village::new().snapshot();
    for format_version in 1..=3 {
        let snapshot = VillageSnapshot {
            format_version,
            ..original.clone()
        };
        assert!(Village::from_snapshot(snapshot).is_ok());
    }
    let mut snapshot = original.clone();
    snapshot.format_version = 4;
    assert!(matches!(
        Village::from_snapshot(snapshot),
        Err(SnapshotError::UnsupportedVersion(4))
    ));
    let mut snapshot = original.clone();
    snapshot.residents[0].hunger = 101;
    assert!(matches!(
        Village::from_snapshot(snapshot),
        Err(SnapshotError::InvalidResidentState)
    ));
    let mut snapshot = original.clone();
    snapshot.residents[0].position = Position::new(65, 0, 0);
    assert!(matches!(
        Village::from_snapshot(snapshot),
        Err(SnapshotError::InvalidResidentState)
    ));
    let mut snapshot = original.clone();
    snapshot.residents[1].id = snapshot.residents[0].id;
    assert!(matches!(
        Village::from_snapshot(snapshot),
        Err(SnapshotError::DuplicateEntityId)
    ));
    let mut snapshot = original;
    snapshot.world_objects[0].quantity = snapshot.world_objects[0].max_quantity + 1;
    assert!(matches!(
        Village::from_snapshot(snapshot),
        Err(SnapshotError::InvalidWorldObjectState)
    ));
}

#[test]
fn stale_and_out_of_bounds_actions_preserve_the_entire_snapshot() {
    for x in [-100, -65, 65, 100] {
        let mut village = Village::new();
        let before = serde_json::to_value(village.snapshot()).unwrap();
        let action = Action::MoveTo {
            destination: Position::new(x, 0, 0),
        };
        assert!(matches!(
            propose(&mut village, action.clone()),
            WorldEvent::ActionRejected {
                reason: RejectionReason::InvalidAction,
                ..
            }
        ));
        assert_eq!(before, serde_json::to_value(village.snapshot()).unwrap());
        let stale = ActionProposal {
            resident: ResidentId(1),
            expected_version: society_domain::WorldVersion(9),
            idempotency_key: "stale".into(),
            action,
        };
        assert!(matches!(
            village.apply(stale),
            WorldEvent::ActionRejected {
                reason: RejectionReason::StaleWorldVersion,
                ..
            }
        ));
        assert_eq!(before, serde_json::to_value(village.snapshot()).unwrap());
    }
}

#[test]
fn sharing_conserves_inventory_across_quantities() {
    for amount in 1..=100 {
        let mut snapshot = Village::new().snapshot();
        snapshot.residents[0]
            .inventory
            .insert(ResourceKind::Wood, amount);
        let mut village = Village::from_snapshot(snapshot).unwrap();
        let before = village
            .residents()
            .iter()
            .map(|r| r.inventory.get(&ResourceKind::Wood).copied().unwrap_or(0))
            .sum::<u32>();
        assert!(matches!(
            propose(
                &mut village,
                Action::ShareResource {
                    to: ResidentId(2),
                    resource: ResourceKind::Wood,
                    amount
                }
            ),
            WorldEvent::ActionCommitted { .. }
        ));
        let after = village
            .residents()
            .iter()
            .map(|r| r.inventory.get(&ResourceKind::Wood).copied().unwrap_or(0))
            .sum::<u32>();
        assert_eq!(before, after);
    }
}

#[test]
fn deterministic_progression_and_snapshot_round_trip() {
    let mut first = Village::new();
    let mut second = Village::new();
    for _ in 0..48 {
        first.tick().unwrap();
        second.tick().unwrap();
        assert_eq!(
            serde_json::to_value(first.snapshot()).unwrap(),
            serde_json::to_value(second.snapshot()).unwrap()
        );
        assert!(first.residents().iter().all(|r| r.hunger <= 100
            && r.thirst <= 100
            && r.energy <= 100
            && r.health <= 100
            && r.morale <= 100));
    }
    let restored = Village::from_snapshot(first.snapshot()).unwrap();
    assert_eq!(
        serde_json::to_value(first.snapshot()).unwrap(),
        serde_json::to_value(restored.snapshot()).unwrap()
    );
}
