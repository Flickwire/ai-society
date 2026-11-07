//! Validation of resident action proposals against the current world state.

use super::rules::*;
use super::types::*;
use super::village::Village;
use society_domain::{Action, ResourceKind, StructureKind, WorldObjectId};
use std::collections::HashSet;

impl Village {
    pub(super) fn action_is_valid(&self, resident_index: usize, action: &Action) -> bool {
        let resident = &self.residents[resident_index];
        let near_object = |id: WorldObjectId| {
            self.world_objects
                .iter()
                .find(|object| object.id == id)
                .filter(|object| resident.position.distance(object.position) <= 1)
        };
        match action {
            Action::Move { destination } => self.places.iter().any(|place| {
                place.id == *destination && resident.position.distance(place.position) <= 12
            }),
            Action::MoveTo { destination } => {
                valid_position(*destination) && resident.position.distance(*destination) <= 12
            }
            Action::Observe => true,
            Action::Speak { to, text } => self.residents.iter().any(|other| {
                other.id == *to
                    && other.id != resident.id
                    && resident.position.distance(other.position) <= 4
                    && !text.trim().is_empty()
                    && text.len() <= 500
            }),
            Action::Eat => {
                resident.food > 0
                    || [
                        ResourceKind::Berries,
                        ResourceKind::Grain,
                        ResourceKind::Meat,
                        ResourceKind::Fish,
                    ]
                    .iter()
                    .any(|resource| inventory_count(resident, *resource) > 0)
            }
            Action::EatResource { food, amount } => {
                is_edible(*food) && *amount > 0 && inventory_count(resident, *food) >= *amount
            }
            Action::Drink { source } => near_object(*source)
                .is_some_and(|object| object.kind == WorldObjectKind::WaterSource),
            Action::Rest { duration } => (1..=24).contains(duration),
            Action::Forage { .. } => false,
            Action::Gather { target, amount } => near_object(*target).is_some_and(|object| {
                gather_resource(object.kind).is_some()
                    && *amount > 0
                    && *amount <= object.quantity
                    && can_add_resource(resident, gather_resource(object.kind).unwrap(), *amount)
            }),
            Action::Hunt { target } => near_object(*target).is_some_and(|object| {
                matches!(object.kind, WorldObjectKind::Deer | WorldObjectKind::Rabbit)
                    && object.quantity > 0
                    && can_add_resource(
                        resident,
                        ResourceKind::Meat,
                        if object.kind == WorldObjectKind::Deer {
                            3
                        } else {
                            1
                        },
                    )
            }),
            Action::Fish { target, amount } => near_object(*target).is_some_and(|object| {
                object.kind == WorldObjectKind::FishSchool
                    && *amount > 0
                    && *amount <= object.quantity
                    && can_add_resource(resident, ResourceKind::Fish, *amount)
            }),
            Action::Build {
                structure,
                position,
            } => {
                self.next_object_id < u64::MAX
                    && valid_position(*position)
                    && resident.position.distance(*position) <= 1
                    && structure_cost(*structure)
                        .iter()
                        .all(|cost| inventory_count(resident, cost.resource) >= cost.amount)
                    && !self.world_objects.iter().any(|object| {
                        object.position == *position
                            && matches!(object.kind, WorldObjectKind::Structure(_))
                    })
            }
            Action::Destroy { target } => near_object(*target).is_some_and(|object| {
                object.kind != WorldObjectKind::WaterSource
                    && match object.kind {
                        WorldObjectKind::Tree => {
                            can_add_resource(resident, ResourceKind::Wood, object.quantity)
                        }
                        WorldObjectKind::Rock => {
                            can_add_resource(resident, ResourceKind::Stone, object.quantity)
                        }
                        WorldObjectKind::Structure(StructureKind::FirePit) => {
                            can_add_resource(resident, ResourceKind::Stone, 2)
                        }
                        WorldObjectKind::Structure(_) => {
                            can_add_resource(resident, ResourceKind::Wood, 2)
                        }
                        _ => true,
                    }
            }),
            Action::Dig { position } => {
                self.next_object_id < u64::MAX
                    && valid_position(*position)
                    && resident.position.distance(*position) <= 1
                    && !self.world_objects.iter().any(|object| {
                        object.position == *position && object.kind == WorldObjectKind::Cave
                    })
            }
            Action::Plant { seed, position } => {
                let valid_seed = matches!(
                    seed,
                    ResourceKind::Grain
                        | ResourceKind::GrainSeed
                        | ResourceKind::Berries
                        | ResourceKind::BerrySeed
                );
                let inventory_seed = inventory_count(resident, *seed);
                valid_seed
                    && inventory_seed > 0
                    && self.next_object_id < u64::MAX
                    && valid_position(*position)
                    && resident.position.distance(*position) <= 1
                    && self.world_objects.iter().any(|object| {
                        object.position.distance(*position) <= 1
                            && matches!(
                                object.kind,
                                WorldObjectKind::Structure(StructureKind::Garden)
                                    | WorldObjectKind::GrainPatch
                            )
                    })
            }
            Action::Harvest { target, amount } => near_object(*target).is_some_and(|object| {
                harvest_resource(object.kind).is_some()
                    && *amount > 0
                    && *amount <= object.quantity
                    && can_add_resource(resident, harvest_resource(object.kind).unwrap(), *amount)
            }),
            Action::CraftItem { recipe } => {
                let (costs, output) = recipe_cost(*recipe);
                costs
                    .iter()
                    .all(|cost| inventory_count(resident, cost.resource) >= cost.amount)
                    && can_add_resource(resident, output, 1)
            }
            Action::Craft { recipe } => parse_recipe(recipe).is_some_and(|recipe| {
                let (costs, output) = recipe_cost(recipe);
                costs
                    .iter()
                    .all(|cost| inventory_count(resident, cost.resource) >= cost.amount)
                    && can_add_resource(resident, output, 1)
            }),
            Action::ShareResource {
                to,
                resource,
                amount,
            } => {
                *amount > 0
                    && inventory_count(resident, *resource) >= *amount
                    && self.residents.iter().any(|other| {
                        other.id == *to
                            && other.id != resident.id
                            && resident.position.distance(other.position) <= 2
                            && can_add_resource(other, *resource, *amount)
                    })
            }
            Action::Play {
                activity,
                participants,
                duration,
            } => {
                !activity.trim().is_empty()
                    && activity.len() <= 32
                    && (1..=4).contains(duration)
                    && participants.len() <= 4
                    && participants.iter().copied().collect::<HashSet<_>>().len()
                        == participants.len()
                    && participants.iter().all(|participant| {
                        *participant != resident.id
                            && self.residents.iter().any(|other| {
                                other.id == *participant
                                    && resident.position.distance(other.position) <= 4
                            })
                    })
            }
            Action::SetGoal { goal } => !goal.trim().is_empty() && goal.len() <= 120,
            Action::Invent {
                name,
                description,
                materials,
            } => {
                !name.trim().is_empty()
                    && name.len() <= 60
                    && !description.trim().is_empty()
                    && description.len() <= 300
                    && materials_available(resident, materials)
                    && !self
                        .inventions
                        .iter()
                        .any(|invention| invention.name.eq_ignore_ascii_case(name.trim()))
            }
            Action::ProposeCapability {
                name,
                description,
                example,
            } => {
                self.next_capability_id < u64::MAX
                    && !name.trim().is_empty()
                    && name.len() <= 60
                    && !description.trim().is_empty()
                    && description.len() <= 300
                    && !example.trim().is_empty()
                    && example.len() <= 300
                    && !self
                        .capability_requests
                        .iter()
                        .any(|request| request.name.eq_ignore_ascii_case(name.trim()))
            }
            Action::VoteCapability { proposal, .. } => {
                self.capability_requests.iter().any(|request| {
                    request.id == *proposal
                        && request.status == CapabilityStatus::Open
                        && !request
                            .votes
                            .iter()
                            .any(|vote| vote.resident == resident.id)
                })
            }
            Action::Work { .. } => false,
            Action::StartProject { .. }
            | Action::ContinueProject { .. }
            | Action::CancelProject { .. }
            | Action::OfferTrade { .. }
            | Action::AcceptTrade { .. } => false,
        }
    }
}
