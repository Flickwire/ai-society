//! Deterministic simulation primitives, independent of graphics and IO.

mod actions;
mod autonomy;
mod clock;
mod inner_life;
mod personal;
mod rules;
mod types;
mod validation;
mod village;

pub use clock::{ClockError, Scheduled, SimulationClock};
pub use personal::{
    AttitudeKind, MemorySource, PersonalAttitude, PersonalMemory, PersonalNote, Personality,
    PhysicalProfile,
};
pub use types::{
    CapabilityRequest, CapabilityStatus, CapabilityVote, ConversationLine, Invention, Place,
    Resident, WorldObject, WorldObjectKind,
};
pub use village::{SnapshotError, TickReport, Village, VillageSnapshot};
