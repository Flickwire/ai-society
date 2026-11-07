//! Shared world entities and wilderness bootstrap data.

use super::personal::{
    PersonalAttitude, PersonalMemory, PersonalNote, Personality, PhysicalProfile,
};
use serde::{Deserialize, Serialize};
use society_domain::{
    CapabilityProposalId, PlaceId, Position, ResidentId, ResourceAmount, ResourceKind,
    StructureKind, VirtualTime, WorldObjectId,
};
use std::collections::BTreeMap;

/// A location in the starter village.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Place {
    /// Stable location identifier.
    pub id: PlaceId,
    /// Human-readable name.
    pub name: String,
    /// Landmark coordinate in the wilderness.
    #[serde(default)]
    pub position: Position,
}

/// A resident and the basic needs tracked by the starter simulation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Resident {
    /// Stable resident identifier.
    pub id: ResidentId,
    /// Resident name.
    pub name: String,
    /// Current location.
    pub place: PlaceId,
    /// Free-movement coordinate in the 3D world.
    #[serde(default)]
    pub position: Position,
    /// Hunger from 0 (sated) to 100 (starving).
    pub hunger: u8,
    /// Energy from 0 (exhausted) to 100 (rested).
    pub energy: u8,
    /// Units of food carried.
    pub food: u8,
    /// Thirst from 0 (hydrated) to 100 (severely dehydrated).
    #[serde(default = "default_thirst")]
    pub thirst: u8,
    /// Health from 0 (dead) to 100 (healthy).
    #[serde(default = "default_health")]
    pub health: u8,
    /// Morale from 0 (despairing) to 100 (joyful).
    #[serde(default = "default_morale")]
    pub morale: u8,
    /// Carried resources and crafted goods.
    #[serde(default)]
    pub inventory: BTreeMap<ResourceKind, u32>,
    /// Current personal goal.
    #[serde(default = "default_goal")]
    pub goal: String,
    /// Individual intelligence score. It can shift slowly through learning.
    #[serde(default = "default_intelligence")]
    pub intelligence: u8,
    /// Evolving physical characteristics, initialized without assigning a role.
    #[serde(default)]
    pub physical: PhysicalProfile,
    /// Individual personality tendencies, which can shift through experience.
    #[serde(default)]
    pub personality: Personality,
    /// Private recollections; these may be incomplete, distorted, or mistaken.
    #[serde(default)]
    pub memories: Vec<PersonalMemory>,
    /// Resident-authored durable notes, kept distinct from decaying recollections.
    #[serde(default)]
    pub notes: Vec<PersonalNote>,
    /// Private, revisable views and preferences, including impressions of people.
    #[serde(default)]
    pub attitudes: Vec<PersonalAttitude>,
    /// Progress toward cognitive learning through difficult or novel experiences.
    #[serde(default)]
    pub learning_experience: u8,
}

/// Category of a natural or constructed world object.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "category", content = "structure", rename_all = "snake_case")]
pub enum WorldObjectKind {
    /// Harvestable tree.
    Tree,
    /// Loose stone outcrop.
    Rock,
    /// Freshwater body.
    WaterSource,
    /// Wild edible plant.
    BerryBush,
    /// Wild grain patch.
    GrainPatch,
    /// Plant fiber source.
    FiberPlant,
    /// Deposit of workable clay.
    ClayBank,
    /// Cave entrance or excavated shelter.
    Cave,
    /// Huntable deer.
    Deer,
    /// Huntable rabbit.
    Rabbit,
    /// Fishable water animal group.
    FishSchool,
    /// Planted crop that grows over virtual time.
    Crop(ResourceKind),
    /// Resident-built structure.
    Structure(StructureKind),
}

/// Natural resource, animal, crop, or structure at a 3D coordinate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorldObject {
    /// Stable world object identifier.
    pub id: WorldObjectId,
    /// Name displayed in observations.
    pub name: String,
    /// Object category.
    pub kind: WorldObjectKind,
    /// World coordinate.
    pub position: Position,
    /// Gatherable stock, crop maturity, or animal health.
    pub quantity: u32,
    /// Initial/regrowth ceiling for renewable resources.
    pub max_quantity: u32,
}

/// State of a community request for a capability missing from the simulator.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    /// Voting remains open.
    Open,
    /// A majority of residents supports the capability.
    Accepted,
    /// A majority of residents rejects the capability.
    Declined,
}

/// One resident's vote on a requested simulator capability.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CapabilityVote {
    /// Resident who cast the ballot.
    pub resident: ResidentId,
    /// Whether the resident supports implementing the capability.
    pub support: bool,
}

/// Agent-submitted request for a new simulator action, with community votes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CapabilityRequest {
    /// Stable request identifier.
    pub id: CapabilityProposalId,
    /// Proposed action name.
    pub name: String,
    /// What it would let the residents do.
    pub description: String,
    /// Example need or situation.
    pub example: String,
    /// Resident who requested it.
    pub requested_by: ResidentId,
    /// Virtual timestamp when it was submitted.
    pub created_at: VirtualTime,
    /// Votes cast so far, at most one per resident.
    pub votes: Vec<CapabilityVote>,
    /// Current majority result.
    pub status: CapabilityStatus,
}

/// Recorded communal invention made by combining materials and an idea.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Invention {
    /// Name chosen by its inventor.
    pub name: String,
    /// The problem it solves or how it works.
    pub description: String,
    /// Resident who first recorded it.
    pub inventor: ResidentId,
    /// Virtual timestamp when it was recorded.
    pub created_at: VirtualTime,
    /// Consumed materials used in experimentation.
    pub materials: Vec<ResourceAmount>,
}

/// Recent message exchanged by residents.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversationLine {
    /// Resident who spoke.
    pub from: ResidentId,
    /// Intended listener.
    pub to: ResidentId,
    /// Spoken text.
    pub text: String,
    /// Virtual timestamp.
    pub at: VirtualTime,
}

pub(super) fn default_thirst() -> u8 {
    20
}

pub(super) fn default_health() -> u8 {
    100
}

pub(super) fn default_morale() -> u8 {
    60
}

pub(super) fn default_goal() -> String {
    "find food and fresh water".to_owned()
}

pub(super) fn default_intelligence() -> u8 {
    50
}

pub(super) fn wilderness_landmarks() -> Vec<Place> {
    [
        ("Open Meadow", Position::new(0, 0, 0)),
        ("River Bend", Position::new(12, 0, -3)),
        ("Berry Hollow", Position::new(-5, 0, 8)),
        ("Pine Ridge", Position::new(-12, 3, -9)),
        ("Cave Mouth", Position::new(-18, 1, 4)),
        ("Wetland", Position::new(8, 0, 12)),
        ("Old Oak", Position::new(4, 0, 3)),
        ("Stone Shelf", Position::new(-6, 2, -12)),
        ("North Grove", Position::new(0, 0, -18)),
        ("High Bluff", Position::new(18, 5, 9)),
        ("Grain Clearing", Position::new(16, 0, -10)),
        ("Camp Hollow", Position::new(-2, 0, -4)),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (name, position))| Place {
        id: PlaceId(index as u64 + 1),
        name: name.to_owned(),
        position,
    })
    .collect()
}

pub(super) fn wilderness_objects() -> Vec<WorldObject> {
    let data = [
        ("pine tree", WorldObjectKind::Tree, -8, 0, -4, 8),
        ("pine tree", WorldObjectKind::Tree, -10, 0, -6, 8),
        ("oak tree", WorldObjectKind::Tree, 3, 0, 3, 10),
        ("fallen cedar", WorldObjectKind::Tree, -13, 0, 2, 6),
        ("granite outcrop", WorldObjectKind::Rock, -5, 1, -11, 12),
        ("river", WorldObjectKind::WaterSource, 9, 0, -2, u32::MAX),
        ("berry bush", WorldObjectKind::BerryBush, -4, 0, -2, 8),
        ("berry bush", WorldObjectKind::BerryBush, -6, 0, 7, 10),
        ("wild grain", WorldObjectKind::GrainPatch, 14, 0, -9, 8),
        ("wild grain", WorldObjectKind::GrainPatch, 18, 0, -11, 6),
        ("flax and reeds", WorldObjectKind::FiberPlant, 7, 0, 10, 12),
        ("clay bank", WorldObjectKind::ClayBank, 11, 0, 0, 10),
        ("shallow cave", WorldObjectKind::Cave, -18, 1, 4, 1),
        ("deer", WorldObjectKind::Deer, 5, 0, 7, 3),
        ("rabbit", WorldObjectKind::Rabbit, -3, 0, 5, 1),
        ("fish shoal", WorldObjectKind::FishSchool, 9, 0, -3, 12),
        ("fish shoal", WorldObjectKind::FishSchool, 12, 0, 1, 8),
        ("dry branch pile", WorldObjectKind::Tree, 1, 0, -6, 3),
        ("boulder", WorldObjectKind::Rock, 17, 2, 7, 8),
        ("highland cave", WorldObjectKind::Cave, 19, 5, 10, 1),
    ];
    data.into_iter()
        .enumerate()
        .map(|(index, (name, kind, x, y, z, quantity))| WorldObject {
            id: WorldObjectId(index as u64 + 1),
            name: name.to_owned(),
            kind,
            position: Position::new(x, y, z),
            quantity,
            max_quantity: quantity,
        })
        .collect()
}
