//! Village state, initialization, persistence snapshots, and observation accessors.

use super::clock::SimulationClock;
use super::rules::*;
use super::types::*;
use serde::{Deserialize, Serialize};
use society_domain::{
    PlaceId, Position, ResidentId, ResourceKind, VirtualTime, WorldObjectId, WorldVersion,
};
use std::collections::{BTreeMap, HashSet};

/// Result of one deterministic simulation tick.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TickReport {
    /// Virtual time after advancing.
    pub time: VirtualTime,
    /// Brief descriptions of autonomous actions taken this tick.
    pub actions: Vec<String>,
}

/// Runnable, deterministic starter village.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Village {
    pub(super) clock: SimulationClock,
    pub(super) places: Vec<Place>,
    pub(super) residents: Vec<Resident>,
    #[serde(default)]
    pub(super) world_objects: Vec<WorldObject>,
    #[serde(default)]
    pub(super) capability_requests: Vec<CapabilityRequest>,
    #[serde(default)]
    pub(super) inventions: Vec<Invention>,
    #[serde(default)]
    pub(super) conversations: Vec<ConversationLine>,
    #[serde(default)]
    pub(super) next_object_id: u64,
    #[serde(default)]
    pub(super) next_capability_id: u64,
    pub(super) forage_stock: u32,
    pub(super) committed_keys: HashSet<String>,
}

/// Versioned checkpoint of authoritative simulation state.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VillageSnapshot {
    /// Snapshot format version.
    pub format_version: u32,
    /// Simulation clock and world version.
    pub clock: SimulationClock,
    /// Locations in the world.
    pub places: Vec<Place>,
    /// Residents and their state.
    pub residents: Vec<Resident>,
    /// Natural and constructed objects in the 3D wilderness.
    #[serde(default)]
    pub world_objects: Vec<WorldObject>,
    /// Capability proposals and agent ballots.
    #[serde(default)]
    pub capability_requests: Vec<CapabilityRequest>,
    /// Inventions recorded by residents.
    #[serde(default)]
    pub inventions: Vec<Invention>,
    /// Recent conversations retained for resident perception.
    #[serde(default)]
    pub conversations: Vec<ConversationLine>,
    /// Next world object ID to allocate.
    #[serde(default)]
    pub next_object_id: u64,
    /// Next capability proposal ID to allocate.
    #[serde(default)]
    pub next_capability_id: u64,
    /// Remaining food available to forage.
    pub forage_stock: u32,
}

/// Snapshot validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotError {
    /// The checkpoint uses a newer or unsupported format.
    UnsupportedVersion(u32),
    /// A world must contain at least one place and resident.
    EmptyWorld,
    /// Place or resident IDs must be unique.
    DuplicateEntityId,
    /// A resident references a missing place or has an out-of-range need.
    InvalidResidentState,
    /// An object identifier, coordinate, or stock value is invalid.
    InvalidWorldObjectState,
    /// Capability requests, ballots, inventions, or dialogue reference invalid residents.
    InvalidSocialState,
}

impl Default for Village {
    fn default() -> Self {
        Self::new()
    }
}

impl Village {
    /// Creates a wilderness with eight residents, twelve landmarks, and natural resources.
    #[must_use]
    pub fn new() -> Self {
        let places = wilderness_landmarks();
        let world_objects = wilderness_objects();
        let names = ["Ada", "Bram", "Cleo", "Dara", "Eli", "Farah", "Gus", "Hana"];
        let residents = names
            .into_iter()
            .enumerate()
            .map(|(index, name)| {
                let position = Position::new(-2 + (index as i32 % 3), 0, -4 + (index as i32 / 3));
                Resident {
                    id: ResidentId(index as u64 + 1),
                    name: name.to_owned(),
                    place: PlaceId(12),
                    position,
                    hunger: 20 + (index as u8 * 3),
                    energy: 70,
                    food: 0,
                    thirst: 20,
                    health: 100,
                    morale: 60,
                    inventory: BTreeMap::from([
                        (ResourceKind::Berries, 1),
                        (ResourceKind::Fiber, 1),
                    ]),
                    goal: [
                        "find a reliable source of food",
                        "find fresh water",
                        "make a safe place to sleep",
                        "discover useful materials",
                    ][index % 4]
                        .to_owned(),
                }
            })
            .collect();
        Self {
            clock: SimulationClock::default(),
            places,
            residents,
            world_objects,
            capability_requests: Vec::new(),
            inventions: Vec::new(),
            conversations: Vec::new(),
            next_object_id: 1 + wilderness_objects().len() as u64,
            next_capability_id: 1,
            forage_stock: 40,
            committed_keys: HashSet::new(),
        }
    }

    /// Current virtual time.
    #[must_use]
    pub const fn now(&self) -> VirtualTime {
        self.clock.now()
    }

    /// Current authoritative world version.
    #[must_use]
    pub const fn version(&self) -> WorldVersion {
        self.clock.version()
    }

    /// Exports a persistence checkpoint.
    #[must_use]
    pub fn snapshot(&self) -> VillageSnapshot {
        VillageSnapshot {
            format_version: 2,
            clock: self.clock.clone(),
            places: self.places.clone(),
            residents: self.residents.clone(),
            world_objects: self.world_objects.clone(),
            capability_requests: self.capability_requests.clone(),
            inventions: self.inventions.clone(),
            conversations: self.conversations.clone(),
            next_object_id: self.next_object_id,
            next_capability_id: self.next_capability_id,
            forage_stock: self.forage_stock,
        }
    }

    /// Restores a village after validating entity references and needs.
    pub fn from_snapshot(snapshot: VillageSnapshot) -> Result<Self, SnapshotError> {
        if !(1..=2).contains(&snapshot.format_version) {
            return Err(SnapshotError::UnsupportedVersion(snapshot.format_version));
        }
        if snapshot.places.is_empty() || snapshot.residents.is_empty() {
            return Err(SnapshotError::EmptyWorld);
        }
        let place_ids: HashSet<_> = snapshot.places.iter().map(|place| place.id).collect();
        let resident_ids: HashSet<_> = snapshot
            .residents
            .iter()
            .map(|resident| resident.id)
            .collect();
        if place_ids.len() != snapshot.places.len()
            || resident_ids.len() != snapshot.residents.len()
        {
            return Err(SnapshotError::DuplicateEntityId);
        }
        if snapshot.residents.iter().any(|resident| {
            !place_ids.contains(&resident.place)
                || !valid_position(resident.position)
                || resident.hunger > 100
                || resident.thirst > 100
                || resident.energy > 100
                || resident.health > 100
                || resident.morale > 100
                || resident.goal.len() > 120
                || resident
                    .inventory
                    .values()
                    .any(|count| *count > MAX_INVENTORY_PER_RESOURCE)
        }) {
            return Err(SnapshotError::InvalidResidentState);
        }
        let world_object_ids: HashSet<_> = snapshot
            .world_objects
            .iter()
            .map(|object| object.id)
            .collect();
        if world_object_ids.len() != snapshot.world_objects.len()
            || snapshot.world_objects.iter().any(|object| {
                !valid_position(object.position) || object.quantity > object.max_quantity
            })
        {
            return Err(SnapshotError::InvalidWorldObjectState);
        }
        if snapshot.capability_requests.iter().any(|request| {
            !resident_ids.contains(&request.requested_by)
                || request
                    .votes
                    .iter()
                    .any(|vote| !resident_ids.contains(&vote.resident))
                || request
                    .votes
                    .iter()
                    .map(|vote| vote.resident)
                    .collect::<HashSet<_>>()
                    .len()
                    != request.votes.len()
        }) || snapshot
            .inventions
            .iter()
            .any(|invention| !resident_ids.contains(&invention.inventor))
            || snapshot.conversations.iter().any(|line| {
                !resident_ids.contains(&line.from)
                    || !resident_ids.contains(&line.to)
                    || line.text.len() > 500
            })
        {
            return Err(SnapshotError::InvalidSocialState);
        }
        let mut places = snapshot.places;
        if snapshot.format_version == 1 {
            let landmarks = wilderness_landmarks();
            for place in &mut places {
                if let Some(landmark) = landmarks.iter().find(|landmark| landmark.id == place.id) {
                    place.position = landmark.position;
                }
            }
        }
        let place_positions: BTreeMap<_, _> = places
            .iter()
            .map(|place| (place.id, place.position))
            .collect();
        let mut residents = snapshot.residents;
        for resident in &mut residents {
            if snapshot.format_version == 1 {
                resident.position = place_positions
                    .get(&resident.place)
                    .copied()
                    .unwrap_or_default();
                if resident.inventory.is_empty() && resident.food > 0 {
                    resident
                        .inventory
                        .insert(ResourceKind::Berries, u32::from(resident.food));
                }
                resident.food = 0;
            }
        }
        let world_objects = if snapshot.format_version == 1 && snapshot.world_objects.is_empty() {
            wilderness_objects()
        } else {
            snapshot.world_objects
        };
        let next_object_id = snapshot.next_object_id.max(
            world_objects
                .iter()
                .map(|object| object.id.0)
                .max()
                .unwrap_or(0)
                + 1,
        );
        Ok(Self {
            clock: snapshot.clock,
            places,
            residents,
            world_objects,
            capability_requests: snapshot.capability_requests,
            inventions: snapshot.inventions,
            conversations: snapshot.conversations,
            next_object_id,
            next_capability_id: snapshot.next_capability_id.max(1),
            forage_stock: snapshot.forage_stock,
            committed_keys: HashSet::new(),
        })
    }

    /// Village locations.
    #[must_use]
    pub fn places(&self) -> &[Place] {
        &self.places
    }

    /// Village residents.
    #[must_use]
    pub fn residents(&self) -> &[Resident] {
        &self.residents
    }

    /// Natural resources, wildlife, crops, caves, and resident-built structures.
    #[must_use]
    pub fn world_objects(&self) -> &[WorldObject] {
        &self.world_objects
    }

    /// Capability requests submitted by residents, including current vote totals.
    #[must_use]
    pub fn capability_requests(&self) -> &[CapabilityRequest] {
        &self.capability_requests
    }

    /// Inventions recorded by residents.
    #[must_use]
    pub fn inventions(&self) -> &[Invention] {
        &self.inventions
    }

    /// Recent resident-to-resident speech.
    #[must_use]
    pub fn conversations(&self) -> &[ConversationLine] {
        &self.conversations
    }

    pub(super) fn nearest_place(&self, position: Position) -> PlaceId {
        self.places
            .iter()
            .min_by_key(|place| position.distance(place.position))
            .map_or(PlaceId(1), |place| place.id)
    }

    pub(super) fn insert_object(
        &mut self,
        name: String,
        kind: WorldObjectKind,
        position: Position,
        quantity: u32,
        max_quantity: u32,
    ) {
        let id = WorldObjectId(self.next_object_id);
        self.next_object_id = self.next_object_id.saturating_add(1);
        self.world_objects.push(WorldObject {
            id,
            name,
            kind,
            position,
            quantity,
            max_quantity,
        });
    }
}
