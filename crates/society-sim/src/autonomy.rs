//! Minimal trait- and need-sensitive policy for deterministic no-model runs.

use super::types::{Resident, WorldObjectKind};
use super::village::Village;
use society_domain::{Action, Position, ResourceKind};

pub(super) fn choose_action(village: &Village, resident: &Resident) -> Action {
    if resident.thirst >= 55 {
        if let Some(water) = village
            .world_objects()
            .iter()
            .filter(|object| object.kind == WorldObjectKind::WaterSource)
            .min_by_key(|object| resident.position.distance(object.position))
        {
            if resident.position.distance(water.position) <= 1 {
                return Action::Drink { source: water.id };
            }
            return Action::MoveTo {
                destination: step_toward(resident.position, water.position),
            };
        }
    }
    if resident.hunger >= 55 {
        if resident.food > 0
            || resident.inventory.iter().any(|(resource, count)| {
                *count > 0
                    && matches!(
                        resource,
                        ResourceKind::Berries
                            | ResourceKind::Grain
                            | ResourceKind::Meat
                            | ResourceKind::Fish
                    )
            })
        {
            return Action::Eat;
        }
        if let Some(food) = village
            .world_objects()
            .iter()
            .filter(|object| {
                object.quantity > 0
                    && matches!(
                        object.kind,
                        WorldObjectKind::BerryBush | WorldObjectKind::GrainPatch
                    )
            })
            .min_by_key(|object| resident.position.distance(object.position))
        {
            if resident.position.distance(food.position) <= 1 {
                return Action::Gather {
                    target: food.id,
                    amount: 1,
                };
            }
            return Action::MoveTo {
                destination: step_toward(resident.position, food.position),
            };
        }
    }
    if resident.energy <= 25 || (resident.health < 50 && resident.energy < 70) {
        return Action::Rest { duration: 4 };
    }

    if resident.personality.extraversion >= 70 && resident.morale < 70 {
        let nearby: Vec<_> = village
            .residents()
            .iter()
            .filter(|other| {
                other.id != resident.id && resident.position.distance(other.position) <= 4
            })
            .map(|other| other.id)
            .take(2)
            .collect();
        if !nearby.is_empty() {
            return Action::Play {
                activity: "play".to_owned(),
                participants: nearby,
                duration: 1,
            };
        }
    }

    if resident.personality.openness >= 65 && resident.energy >= 35 {
        let direction = (resident.id.0.wrapping_mul(31) + village.now().0) % 8;
        let (dx, dz) = match direction {
            0 => (6, 0),
            1 => (6, 6),
            2 => (0, 6),
            3 => (-6, 6),
            4 => (-6, 0),
            5 => (-6, -6),
            6 => (0, -6),
            _ => (6, -6),
        };
        return Action::MoveTo {
            destination: Position::new(
                (resident.position.x + dx).clamp(-64, 64),
                resident.position.y,
                (resident.position.z + dz).clamp(-64, 64),
            ),
        };
    }
    Action::Observe
}

fn step_toward(current: Position, target: Position) -> Position {
    Position::new(
        current.x + (target.x - current.x).clamp(-6, 6),
        current.y + (target.y - current.y).clamp(-6, 6),
        current.z + (target.z - current.z).clamp(-6, 6),
    )
}
