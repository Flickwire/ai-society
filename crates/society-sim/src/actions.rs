//! Action validation and authoritative world transitions.

use super::autonomy::choose_action;
use super::clock::ClockError;
use super::inner_life::{
    decay_memories, evolve_from_action, record_action_memories, update_body_from_needs,
};
use super::rules::*;
use super::types::*;
use super::village::{TickReport, Village};
use society_domain::{
    Action, ActionProposal, CapabilityProposalId, RejectionReason, ResourceKind, StructureKind,
    VirtualTime, WorldEvent,
};

impl Village {
    fn consume_food(&mut self, resident_index: usize, amount: u32) {
        for resource in [
            ResourceKind::Berries,
            ResourceKind::Grain,
            ResourceKind::Meat,
            ResourceKind::Fish,
        ] {
            let count = inventory_count(&self.residents[resident_index], resource);
            if count >= amount {
                remove_resource(&mut self.residents[resident_index], resource, amount);
                self.residents[resident_index].hunger = self.residents[resident_index]
                    .hunger
                    .saturating_sub(u8::try_from(amount.saturating_mul(25)).unwrap_or(u8::MAX));
                return;
            }
        }
        self.residents[resident_index].food = self.residents[resident_index]
            .food
            .saturating_sub(amount as u8);
        self.residents[resident_index].hunger =
            self.residents[resident_index].hunger.saturating_sub(45);
    }

    fn remove_resource(&mut self, resident_index: usize, resource: ResourceKind, amount: u32) {
        remove_resource(&mut self.residents[resident_index], resource, amount);
    }

    /// Advances one hour and applies a simple need- and trait-sensitive policy.
    pub fn tick(&mut self) -> Result<TickReport, ClockError> {
        let mut report = self.advance_for_decisions()?;
        let ids: Vec<_> = self.residents.iter().map(|resident| resident.id).collect();
        for id in ids {
            let Some(resident) = self
                .residents
                .iter()
                .find(|resident| resident.id == id)
                .cloned()
            else {
                continue;
            };
            let action = choose_action(self, &resident);
            let event = self.apply(ActionProposal {
                resident: id,
                expected_version: self.version(),
                idempotency_key: format!("autonomy-{}-{}", id.0, report.time.0),
                action,
            });
            if let WorldEvent::ActionCommitted { action, .. } = event {
                report
                    .actions
                    .push(format!("{}: {}", resident.name, action_label(&action)));
            }
        }
        Ok(report)
    }

    /// Advances needs and virtual time without applying an autonomous policy.
    ///
    /// This is the decision boundary used by inference-driven runners.
    pub fn advance_for_decisions(&mut self) -> Result<TickReport, ClockError> {
        let next_time = VirtualTime(
            self.clock
                .now()
                .0
                .checked_add(1)
                .ok_or(ClockError::TimeExhausted)?,
        );
        self.clock.advance_to(next_time)?;
        for resident in &mut self.residents {
            resident.hunger = resident.hunger.saturating_add(4).min(100);
            resident.thirst = resident.thirst.saturating_add(5).min(100);
            resident.energy = resident.energy.saturating_sub(3);
            if resident.hunger >= 90 || resident.thirst >= 90 {
                resident.health = resident.health.saturating_sub(2);
            }
            if resident.health < 50 {
                resident.morale = resident.morale.saturating_sub(1);
            }
        }
        decay_memories(&mut self.residents, next_time);
        if next_time.0 % 24 == 0 {
            update_body_from_needs(&mut self.residents, next_time);
        }
        if next_time.0 % 24 == 0 {
            for object in &mut self.world_objects {
                match object.kind {
                    WorldObjectKind::BerryBush
                    | WorldObjectKind::GrainPatch
                    | WorldObjectKind::FishSchool => {
                        object.quantity =
                            object.quantity.saturating_add(1).min(object.max_quantity);
                    }
                    WorldObjectKind::Crop(_) if object.quantity < object.max_quantity => {
                        object.quantity += 1;
                    }
                    _ => {}
                }
            }
        }
        if next_time.0 % 6 == 0 {
            for object in &mut self.world_objects {
                if matches!(object.kind, WorldObjectKind::Deer | WorldObjectKind::Rabbit) {
                    let step = (next_time.0 / 6 + object.id.0) % 4;
                    match step {
                        0 => object.position.x += 1,
                        1 => object.position.z -= 1,
                        2 => object.position.x -= 1,
                        _ => object.position.z += 1,
                    }
                }
            }
        }
        self.clock.commit()?;
        Ok(TickReport {
            time: next_time,
            actions: Vec::new(),
        })
    }

    /// Advances by the requested number of hours and returns the final report.
    pub fn run_for(&mut self, hours: u64) -> Result<Vec<TickReport>, ClockError> {
        let capacity = usize::try_from(hours).unwrap_or(usize::MAX).min(100_000);
        let mut reports = Vec::with_capacity(capacity);
        for _ in 0..hours {
            reports.push(self.tick()?);
        }
        Ok(reports)
    }

    /// Applies one typed proposal through the authoritative single-writer rules.
    pub fn apply(&mut self, proposal: ActionProposal) -> WorldEvent {
        let Some(index) = self
            .residents
            .iter()
            .position(|resident| resident.id == proposal.resident)
        else {
            return WorldEvent::ActionRejected {
                resident: proposal.resident,
                reason: RejectionReason::UnknownEntity,
            };
        };
        let reject = |reason| WorldEvent::ActionRejected {
            resident: proposal.resident,
            reason,
        };
        if proposal.expected_version != self.clock.version() {
            return reject(RejectionReason::StaleWorldVersion);
        }
        if proposal.idempotency_key.trim().is_empty() {
            return reject(RejectionReason::InvalidAction);
        }
        if self.committed_keys.contains(&proposal.idempotency_key) {
            return reject(RejectionReason::DuplicateAction);
        }
        if self.clock.version().0 == u64::MAX {
            return reject(RejectionReason::InvalidAction);
        }

        if !self.action_is_valid(index, &proposal.action) {
            return reject(RejectionReason::InvalidAction);
        }

        let actor_position = self.residents[index].position;
        let observers: Vec<_> = self
            .residents
            .iter()
            .filter(|resident| {
                resident.id != proposal.resident && resident.position.distance(actor_position) <= 8
            })
            .map(|resident| resident.id)
            .collect();
        let event_time = self.clock.now();
        record_action_memories(
            &mut self.residents,
            proposal.resident,
            &proposal.action,
            &observers,
            event_time,
        );

        match &proposal.action {
            Action::Move { destination } => {
                if let Some(place) = self.places.iter().find(|place| place.id == *destination) {
                    self.residents[index].place = *destination;
                    self.residents[index].position = place.position;
                }
            }
            Action::MoveTo { destination } => {
                self.residents[index].position = *destination;
                self.residents[index].place = self.nearest_place(*destination);
            }
            Action::Observe => {}
            Action::Speak { to, text } => {
                if let Some(other) = self.residents.iter_mut().find(|other| other.id == *to) {
                    other.morale = other.morale.saturating_add(1).min(100);
                }
                self.residents[index].morale =
                    self.residents[index].morale.saturating_add(1).min(100);
                self.conversations.push(ConversationLine {
                    from: proposal.resident,
                    to: *to,
                    text: text.clone(),
                    at: self.clock.now(),
                });
                if self.conversations.len() > 1_000 {
                    self.conversations.remove(0);
                }
            }
            Action::Eat => self.consume_food(index, 1),
            Action::EatResource { food, amount } => {
                self.remove_resource(index, *food, *amount);
                self.residents[index].hunger = self.residents[index]
                    .hunger
                    .saturating_sub(u8::try_from(amount.saturating_mul(25)).unwrap_or(u8::MAX));
            }
            Action::Drink { .. } => {
                self.residents[index].thirst = self.residents[index].thirst.saturating_sub(60);
            }
            Action::Rest { duration } => {
                let recovery = duration.saturating_mul(18).min(100);
                self.residents[index].energy =
                    (u64::from(self.residents[index].energy) + recovery).min(100) as u8;
                if *duration >= 4 {
                    self.residents[index].health =
                        self.residents[index].health.saturating_add(2).min(100);
                }
            }
            Action::Forage { .. } => {}
            Action::Gather { target, amount } => {
                if let Some(object) = self
                    .world_objects
                    .iter_mut()
                    .find(|object| object.id == *target)
                {
                    let resource = gather_resource(object.kind).expect("validated gather resource");
                    object.quantity -= amount;
                    add_resource(&mut self.residents[index], resource, *amount);
                }
            }
            Action::Hunt { target } => {
                if let Some(target_index) = self
                    .world_objects
                    .iter()
                    .position(|object| object.id == *target)
                {
                    let meat = if self.world_objects[target_index].kind == WorldObjectKind::Deer {
                        3
                    } else {
                        1
                    };
                    self.world_objects.remove(target_index);
                    add_resource(&mut self.residents[index], ResourceKind::Meat, meat);
                    self.residents[index].morale =
                        self.residents[index].morale.saturating_add(2).min(100);
                }
            }
            Action::Fish { target, amount } => {
                if let Some(object) = self
                    .world_objects
                    .iter_mut()
                    .find(|object| object.id == *target)
                {
                    object.quantity -= amount;
                    add_resource(&mut self.residents[index], ResourceKind::Fish, *amount);
                }
            }
            Action::Build {
                structure,
                position,
            } => {
                for cost in structure_cost(*structure) {
                    self.remove_resource(index, cost.resource, cost.amount);
                }
                self.insert_object(
                    structure_name(*structure),
                    WorldObjectKind::Structure(*structure),
                    *position,
                    1,
                    1,
                );
            }
            Action::Destroy { target } => {
                if let Some(target_index) = self
                    .world_objects
                    .iter()
                    .position(|object| object.id == *target)
                {
                    let object = self.world_objects.remove(target_index);
                    let refund = match object.kind {
                        WorldObjectKind::Tree => Some((ResourceKind::Wood, object.quantity.max(1))),
                        WorldObjectKind::Rock => {
                            Some((ResourceKind::Stone, object.quantity.max(1)))
                        }
                        WorldObjectKind::Structure(StructureKind::FirePit) => {
                            Some((ResourceKind::Stone, 2))
                        }
                        WorldObjectKind::Structure(_) => Some((ResourceKind::Wood, 2)),
                        _ => None,
                    };
                    if let Some((resource, amount)) = refund {
                        add_resource(&mut self.residents[index], resource, amount);
                    }
                }
            }
            Action::Dig { position } => {
                self.insert_object(
                    "dug shelter".to_owned(),
                    WorldObjectKind::Cave,
                    *position,
                    1,
                    1,
                );
            }
            Action::Plant { seed, position } => {
                self.remove_resource(index, *seed, 1);
                let crop = if matches!(seed, ResourceKind::GrainSeed | ResourceKind::Grain) {
                    ResourceKind::Grain
                } else {
                    ResourceKind::Berries
                };
                self.insert_object(
                    format!("planted {crop:?}"),
                    WorldObjectKind::Crop(crop),
                    *position,
                    0,
                    4,
                );
            }
            Action::Harvest { target, amount } => {
                if let Some(object) = self
                    .world_objects
                    .iter_mut()
                    .find(|object| object.id == *target)
                {
                    let resource =
                        harvest_resource(object.kind).expect("validated harvest resource");
                    object.quantity -= amount;
                    add_resource(&mut self.residents[index], resource, *amount);
                }
            }
            Action::CraftItem { recipe } => {
                let (costs, output) = recipe_cost(*recipe);
                for cost in costs {
                    self.remove_resource(index, cost.resource, cost.amount);
                }
                add_resource(&mut self.residents[index], output, 1);
            }
            Action::Craft { recipe } => {
                if let Some(recipe) = parse_recipe(recipe) {
                    let (costs, output) = recipe_cost(recipe);
                    for cost in costs {
                        self.remove_resource(index, cost.resource, cost.amount);
                    }
                    add_resource(&mut self.residents[index], output, 1);
                }
            }
            Action::ShareResource {
                to,
                resource,
                amount,
            } => {
                self.remove_resource(index, *resource, *amount);
                if let Some(other) = self.residents.iter_mut().find(|other| other.id == *to) {
                    *other.inventory.entry(*resource).or_default() += amount;
                    other.morale = other.morale.saturating_add(2).min(100);
                }
                self.residents[index].morale =
                    self.residents[index].morale.saturating_add(1).min(100);
            }
            Action::Play {
                participants,
                duration,
                ..
            } => {
                for participant in participants {
                    if let Some(other) = self
                        .residents
                        .iter_mut()
                        .find(|other| other.id == *participant)
                    {
                        other.morale = other.morale.saturating_add(8).min(100);
                    }
                }
                self.residents[index].morale =
                    self.residents[index].morale.saturating_add(8).min(100);
                self.residents[index].energy = self.residents[index]
                    .energy
                    .saturating_sub(u8::try_from(duration.saturating_mul(2)).unwrap_or(u8::MAX));
            }
            Action::SetGoal { goal } => self.residents[index].goal = goal.trim().to_owned(),
            Action::Invent {
                name,
                description,
                materials,
            } => {
                for material in materials {
                    self.remove_resource(index, material.resource, material.amount);
                }
                self.inventions.push(Invention {
                    name: name.trim().to_owned(),
                    description: description.trim().to_owned(),
                    inventor: proposal.resident,
                    created_at: self.clock.now(),
                    materials: materials.clone(),
                });
                self.residents[index].morale =
                    self.residents[index].morale.saturating_add(5).min(100);
            }
            Action::ProposeCapability {
                name,
                description,
                example,
            } => {
                let request = CapabilityRequest {
                    id: CapabilityProposalId(self.next_capability_id),
                    name: name.trim().to_owned(),
                    description: description.trim().to_owned(),
                    example: example.trim().to_owned(),
                    requested_by: proposal.resident,
                    created_at: self.clock.now(),
                    votes: Vec::new(),
                    status: CapabilityStatus::Open,
                };
                self.next_capability_id = self.next_capability_id.saturating_add(1);
                self.capability_requests.push(request);
            }
            Action::VoteCapability {
                proposal: id,
                support,
            } => {
                if let Some(request) = self
                    .capability_requests
                    .iter_mut()
                    .find(|request| request.id == *id)
                {
                    if let Some(vote) = request
                        .votes
                        .iter_mut()
                        .find(|vote| vote.resident == proposal.resident)
                    {
                        vote.support = *support;
                    } else {
                        request.votes.push(CapabilityVote {
                            resident: proposal.resident,
                            support: *support,
                        });
                    }
                    let support_count = request.votes.iter().filter(|vote| vote.support).count();
                    let oppose_count = request.votes.len() - support_count;
                    let majority = self.residents.len() / 2 + 1;
                    request.status = if support_count >= majority {
                        CapabilityStatus::Accepted
                    } else if oppose_count >= majority {
                        CapabilityStatus::Declined
                    } else {
                        CapabilityStatus::Open
                    };
                }
            }
            Action::Work { .. } => {}
            Action::StartProject { .. }
            | Action::ContinueProject { .. }
            | Action::CancelProject { .. }
            | Action::OfferTrade { .. }
            | Action::AcceptTrade { .. } => {}
        }
        evolve_from_action(
            &mut self.residents,
            proposal.resident,
            &proposal.action,
            event_time,
        );
        self.committed_keys.insert(proposal.idempotency_key);
        if self.clock.commit().is_err() {
            return reject(RejectionReason::InvalidAction);
        }
        WorldEvent::ActionCommitted {
            resident: proposal.resident,
            action: proposal.action,
        }
    }
}

fn action_label(action: &Action) -> &'static str {
    match action {
        Action::MoveTo { .. } | Action::Move { .. } => "moves",
        Action::Observe => "observes",
        Action::Speak { .. } => "speaks",
        Action::Eat | Action::EatResource { .. } => "eats",
        Action::Drink { .. } => "drinks",
        Action::Rest { .. } => "rests",
        Action::Forage { .. } | Action::Gather { .. } | Action::Harvest { .. } => {
            "gathers food or materials"
        }
        Action::Craft { .. } | Action::CraftItem { .. } => "crafts",
        Action::Work { .. } => "works",
        Action::Hunt { .. } => "hunts",
        Action::Fish { .. } => "fishes",
        Action::Build { .. } => "builds",
        Action::Destroy { .. } => "alters the landscape",
        Action::Dig { .. } => "digs",
        Action::Plant { .. } => "plants",
        Action::ShareResource { .. } => "shares resources",
        Action::Play { .. } => "plays",
        Action::SetGoal { .. } => "sets a personal goal",
        Action::Invent { .. } => "experiments with an invention",
        Action::ProposeCapability { .. } => "proposes a new capability",
        Action::VoteCapability { .. } => "votes on a capability",
        Action::OfferTrade { .. } => "offers a trade",
        Action::AcceptTrade { .. } => "accepts a trade",
        Action::StartProject { .. } => "starts a project",
        Action::ContinueProject { .. } => "continues a project",
        Action::CancelProject { .. } => "cancels a project",
    }
}
