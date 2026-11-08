//! Simulation invariants across accepted and rejected transitions.
use society_domain::{
    Action, ActionProposal, CraftRecipe, Position, RejectionReason, ResidentId, ResourceKind,
    VirtualTime, WorldEvent, WorldVersion,
};
use society_sim::{ClockError, SimulationClock, Village, WorldObjectKind};

fn apply(v: &mut Village, action: Action, key: &str, version: WorldVersion) -> WorldEvent {
    v.apply(ActionProposal {
        resident: ResidentId(1),
        expected_version: version,
        idempotency_key: key.into(),
        action,
    })
}
fn state(v: &Village) -> serde_json::Value {
    serde_json::to_value(v.snapshot()).unwrap()
}

#[test]
fn valid_action_with_stale_version_is_rejected_without_state_change() {
    let mut v = Village::new();
    let before = state(&v);
    let event = apply(
        &mut v,
        Action::Rest { duration: 1 },
        "stale",
        WorldVersion(1),
    );
    assert!(matches!(
        event,
        WorldEvent::ActionRejected {
            reason: RejectionReason::StaleWorldVersion,
            ..
        }
    ));
    assert_eq!(state(&v), before);
    let version = v.version();
    assert!(matches!(
        apply(&mut v, Action::Rest { duration: 1 }, "valid", version),
        WorldEvent::ActionCommitted { .. }
    ));
    assert_eq!(v.version().0, 1);
}

#[test]
fn clock_enforces_monotonic_time_and_checked_versions() {
    let mut clock = SimulationClock::default();
    clock.advance_to(VirtualTime(10)).unwrap();
    assert_eq!(
        clock.advance_to(VirtualTime(9)),
        Err(ClockError::TimeReversal)
    );
    assert_eq!(clock.now(), VirtualTime(10));
    assert_eq!(clock.commit().unwrap(), WorldVersion(1));
    let mut clock: SimulationClock =
        serde_json::from_value(serde_json::json!({"now": 10, "version": u64::MAX})).unwrap();
    assert_eq!(clock.commit(), Err(ClockError::VersionExhausted));
    assert_eq!(clock.version(), WorldVersion(u64::MAX));
}

#[test]
fn needs_advance_and_saturate_at_their_limits() {
    for (hunger, thirst, energy, health, morale) in [
        (0, 0, 0, 100, 100),
        (96, 95, 2, 3, 1),
        (100, 100, 100, 49, 0),
    ] {
        let mut s = Village::new().snapshot();
        let r = &mut s.residents[0];
        r.hunger = hunger;
        r.thirst = thirst;
        r.energy = energy;
        r.health = health;
        r.morale = morale;
        let mut v = Village::from_snapshot(s).unwrap();
        v.advance_for_decisions().unwrap();
        let r = &v.residents()[0];
        assert_eq!(r.hunger, hunger.saturating_add(4).min(100));
        assert_eq!(r.thirst, thirst.saturating_add(5).min(100));
        assert_eq!(r.energy, energy.saturating_sub(3));
        let expected_health = if r.hunger >= 90 || r.thirst >= 90 {
            health.saturating_sub(2)
        } else {
            health
        };
        assert_eq!(r.health, expected_health);
        assert_eq!(
            r.morale,
            if expected_health < 50 {
                morale.saturating_sub(1)
            } else {
                morale
            }
        );
        assert_eq!(v.version().0, 1);
    }
}

#[test]
fn replay_explicit_action_sequence_produces_identical_state_after_every_step() {
    let mut a = Village::new();
    let mut b = Village::new();
    let actions = [
        Action::Rest { duration: 1 },
        Action::SetGoal {
            goal: "Find water".into(),
        },
        Action::KeepNote {
            text: "remember".into(),
        },
        Action::MoveTo {
            destination: Position::new(-1, 0, -4),
        },
        Action::Eat,
    ];
    for (index, action) in actions.into_iter().enumerate() {
        let key = format!("sequence-{index}");
        let va = a.version();
        let vb = b.version();
        assert_eq!(
            apply(&mut a, action.clone(), &key, va),
            apply(&mut b, action, &key, vb)
        );
        assert_eq!(state(&a), state(&b));
        assert_eq!(a.version().0, u64::try_from(index + 1).unwrap());
    }
}

#[test]
fn inventory_limits_and_gathering_preserve_resources() {
    for (inventory, amount, valid) in [
        (9999, 1, true),
        (10000, 1, false),
        (0, 0, false),
        (9999, 2, false),
    ] {
        let mut s = Village::new().snapshot();
        let tree = s
            .world_objects
            .iter()
            .find(|o| o.kind == WorldObjectKind::Tree)
            .unwrap()
            .clone();
        s.residents[0].position = tree.position;
        s.residents[0]
            .inventory
            .insert(ResourceKind::Wood, inventory);
        let mut v = Village::from_snapshot(s).unwrap();
        let before = state(&v);
        let version = v.version();
        let result = apply(
            &mut v,
            Action::Gather {
                target: tree.id,
                amount,
            },
            "gather",
            version,
        );
        if valid {
            assert!(matches!(result, WorldEvent::ActionCommitted { .. }));
            let tree_after = v.world_objects().iter().find(|o| o.id == tree.id).unwrap();
            assert_eq!(tree.quantity - tree_after.quantity, amount);
            assert_eq!(
                v.residents()[0].inventory[&ResourceKind::Wood] - inventory,
                amount
            );
        } else {
            assert!(matches!(result, WorldEvent::ActionRejected { .. }));
            assert_eq!(state(&v), before);
        }
    }
}

#[test]
fn crafting_checks_materials_and_consumes_exact_recipe() {
    for fiber in 0..=5 {
        let mut s = Village::new().snapshot();
        s.residents[0].inventory.insert(ResourceKind::Fiber, fiber);
        let mut v = Village::from_snapshot(s).unwrap();
        let before = state(&v);
        let version = v.version();
        let result = apply(
            &mut v,
            Action::CraftItem {
                recipe: CraftRecipe::Rope,
            },
            "craft",
            version,
        );
        if fiber >= 3 {
            assert!(matches!(result, WorldEvent::ActionCommitted { .. }));
            assert_eq!(
                v.residents()[0]
                    .inventory
                    .get(&ResourceKind::Fiber)
                    .copied()
                    .unwrap_or(0),
                fiber - 3
            );
            assert_eq!(v.residents()[0].inventory[&ResourceKind::Rope], 1);
        } else {
            assert!(matches!(result, WorldEvent::ActionRejected { .. }));
            assert_eq!(state(&v), before);
        }
    }
}
