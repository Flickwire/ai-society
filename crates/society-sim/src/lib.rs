//! Deterministic simulation primitives, independent of graphics and IO.

mod actions;
mod clock;
mod rules;
mod types;
mod validation;
mod village;

pub use clock::{ClockError, Scheduled, SimulationClock};
pub use types::{
    CapabilityRequest, CapabilityStatus, CapabilityVote, ConversationLine, Invention, Place,
    Resident, WorldObject, WorldObjectKind,
};
pub use village::{SnapshotError, TickReport, Village, VillageSnapshot};
