//! Stable types shared by the simulation and its projections.

use serde::{Deserialize, Serialize};

/// Stable identifier for a resident.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ResidentId(pub u64);

/// Stable identifier for a place in the world.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PlaceId(pub u64);

/// Stable identifier for an object in the 3D world.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct WorldObjectId(pub u64);

/// Stable identifier for an unsupported-action proposal.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct CapabilityProposalId(pub u64);

/// Integer block coordinate in the bounded three-dimensional world.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Position {
    /// East/west coordinate.
    pub x: i32,
    /// Elevation coordinate.
    pub y: i32,
    /// North/south coordinate.
    pub z: i32,
}

impl Position {
    /// Creates a coordinate.
    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Chebyshev distance, allowing diagonal movement in the horizontal plane.
    #[must_use]
    pub fn distance(self, other: Self) -> u32 {
        self.x
            .abs_diff(other.x)
            .max(self.y.abs_diff(other.y))
            .max(self.z.abs_diff(other.z))
    }
}

/// Resources and crafted goods carried by residents or stored in structures.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    /// Cut wood or fallen branches.
    Wood,
    /// Loose stone.
    Stone,
    /// Plant fiber.
    Fiber,
    /// Edible berries.
    Berries,
    /// Wild grain.
    Grain,
    /// Animal meat.
    Meat,
    /// Caught fish.
    Fish,
    /// Potable water carried in a vessel.
    Water,
    /// Clay for pottery and building.
    Clay,
    /// Grain seed.
    GrainSeed,
    /// Berry seed.
    BerrySeed,
    /// A stone axe.
    StoneAxe,
    /// A wooden hunting spear.
    Spear,
    /// Woven rope.
    Rope,
    /// Woven basket.
    Basket,
    /// Fishing net.
    Net,
}

/// Amount of a resource used by an action.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceAmount {
    /// Resource kind.
    pub resource: ResourceKind,
    /// Positive quantity.
    pub amount: u32,
}

/// Primitive construction types available to the residents.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StructureKind {
    /// Temporary branch shelter.
    LeanTo,
    /// Fire ring made from stones.
    FirePit,
    /// Shared resource store.
    StorageRack,
    /// Crafting and invention bench.
    Workbench,
    /// Crop plot prepared for planting.
    Garden,
    /// Defensive or animal enclosure.
    Fence,
    /// Small watercraft.
    Raft,
}

/// Fixed recipes residents can learn and craft.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CraftRecipe {
    /// Stone cutting tool.
    StoneAxe,
    /// Hunting spear.
    Spear,
    /// Rope from plant fiber.
    Rope,
    /// Basket for carrying gathered items.
    Basket,
    /// Woven fishing net.
    Net,
}

/// Monotonic version of authoritative world state.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorldVersion(pub u64);

/// Integer virtual time in simulation ticks.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct VirtualTime(pub u64);

/// A validated proposal submitted to the single simulation writer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ActionProposal {
    /// Resident making the proposal.
    pub resident: ResidentId,
    /// State version observed while making the proposal.
    pub expected_version: WorldVersion,
    /// Stable caller-provided key used to make retries idempotent.
    pub idempotency_key: String,
    /// Proposed action.
    pub action: Action,
}

/// Actions that can be proposed by an agent or an external controller.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Move to any valid coordinate in the 3D world.
    MoveTo {
        /// Destination coordinate.
        destination: Position,
    },
    /// Travel to a connected place.
    Move {
        /// Destination place.
        destination: PlaceId,
    },
    /// Inspect the resident's current surroundings.
    Observe,
    /// Say a short line to another resident.
    Speak {
        /// Resident who should hear the line.
        to: ResidentId,
        /// Spoken text, limited to 500 UTF-8 bytes by the current rules.
        text: String,
    },
    /// Consume one unit of food from inventory.
    Eat,
    /// Rest for a number of virtual ticks.
    Rest {
        /// Duration in virtual ticks.
        duration: u64,
    },
    /// Gather a resource at the current place.
    Forage {
        /// Resource name; currently only `food` is available.
        resource: String,
        /// Number of resource units to collect.
        amount: u32,
    },
    /// Make an item from a named recipe.
    Craft {
        /// Named recipe.
        recipe: String,
    },
    /// Perform a named job.
    Work {
        /// Named job.
        job: String,
        /// Duration in virtual ticks.
        duration: u64,
    },
    /// Offer a transfer of goods or currency.
    OfferTrade {
        /// Recipient resident.
        to: ResidentId,
        /// Stable trade offer identifier.
        offer_id: u64,
    },
    /// Accept a previously offered trade.
    AcceptTrade {
        /// Identifier of the offer to accept.
        offer_id: u64,
    },
    /// Begin a shared project.
    StartProject {
        /// Project name.
        project: String,
    },
    /// Contribute to a project.
    ContinueProject {
        /// Project name.
        project: String,
        /// Effort contributed this action.
        effort: u32,
    },
    /// Stop participating in a project.
    CancelProject {
        /// Project name.
        project: String,
    },
    /// Gather a resource from a nearby natural object.
    Gather {
        /// Object identifier.
        target: WorldObjectId,
        /// Number of units to gather.
        amount: u32,
    },
    /// Drink from a nearby water source.
    Drink {
        /// Object identifier of the water source.
        source: WorldObjectId,
    },
    /// Hunt a nearby deer or rabbit.
    Hunt {
        /// Animal object identifier.
        target: WorldObjectId,
    },
    /// Catch fish at a nearby stream or lake.
    Fish {
        /// Fish school object identifier.
        target: WorldObjectId,
        /// Number of fish to catch.
        amount: u32,
    },
    /// Eat a carried food resource.
    EatResource {
        /// Edible inventory item.
        food: ResourceKind,
        /// Number of portions to eat.
        amount: u32,
    },
    /// Build a structure at or beside the resident's position.
    Build {
        /// Structure to build.
        structure: StructureKind,
        /// World coordinate for construction.
        position: Position,
    },
    /// Destroy a nearby structure or natural resource.
    Destroy {
        /// Object identifier to remove or damage.
        target: WorldObjectId,
    },
    /// Dig soil at a nearby coordinate, creating a shallow cave or pit.
    Dig {
        /// Dig coordinate.
        position: Position,
    },
    /// Plant gathered seeds in a nearby garden plot.
    Plant {
        /// Seed resource to plant.
        seed: ResourceKind,
        /// Planting coordinate.
        position: Position,
    },
    /// Harvest a mature nearby crop or berry bush.
    Harvest {
        /// Plant object identifier.
        target: WorldObjectId,
        /// Number of portions to harvest.
        amount: u32,
    },
    /// Craft a known primitive recipe.
    CraftItem {
        /// Recipe to craft.
        recipe: CraftRecipe,
    },
    /// Share goods directly with a nearby resident.
    ShareResource {
        /// Recipient resident.
        to: ResidentId,
        /// Resource kind to give.
        resource: ResourceKind,
        /// Positive amount to transfer.
        amount: u32,
    },
    /// Play, sing, tell stories, or dance with nearby residents.
    Play {
        /// Activity, such as `sing`, `story`, `dance`, or `game`.
        activity: String,
        /// Nearby participants, possibly empty for a solo activity.
        participants: Vec<ResidentId>,
        /// Duration in virtual hours.
        duration: u64,
    },
    /// Choose or update a personal survival, social, or exploration goal.
    SetGoal {
        /// Short natural-language goal.
        goal: String,
    },
    /// Record an invention by combining known materials and proposing a use.
    Invent {
        /// Invention name.
        name: String,
        /// What problem it solves or how it works.
        description: String,
        /// Materials consumed during experimentation.
        materials: Vec<ResourceAmount>,
    },
    /// Request a simulator action that is not currently available.
    ProposeCapability {
        /// Concise proposed action name.
        name: String,
        /// What the action should let residents do.
        description: String,
        /// Example situation where it is needed.
        example: String,
    },
    /// Vote on an unsupported-action proposal.
    VoteCapability {
        /// Proposal identifier.
        proposal: CapabilityProposalId,
        /// Whether this resident wants the capability added.
        support: bool,
    },
}

/// Durable facts emitted by an authoritative state transition.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorldEvent {
    /// An action was accepted and committed.
    ActionCommitted {
        /// Resident whose action was committed.
        resident: ResidentId,
        /// Committed action.
        action: Action,
    },
    /// A proposed action was rejected with a stable machine-readable reason.
    ActionRejected {
        /// Resident whose action was rejected.
        resident: ResidentId,
        /// Stable reason code.
        reason: RejectionReason,
    },
}

/// Stable rejection categories suitable for logs and UI display.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RejectionReason {
    /// Proposal was based on stale state.
    StaleWorldVersion,
    /// Idempotency key has already been committed.
    DuplicateAction,
    /// Proposed parameters are invalid.
    InvalidAction,
    /// Referenced resident or place does not exist.
    UnknownEntity,
}
