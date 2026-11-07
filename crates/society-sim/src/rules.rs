//! Deterministic spatial, inventory, gathering, crafting, and construction rules.

use super::types::{Resident, WorldObjectKind};
use society_domain::{CraftRecipe, Position, ResourceAmount, ResourceKind, StructureKind};
use std::collections::BTreeMap;

const MAX_WORLD_COORDINATE: i32 = 64;
const MAX_WORLD_HEIGHT: i32 = 16;
pub(super) const MAX_INVENTORY_PER_RESOURCE: u32 = 10_000;

pub(super) fn valid_position(position: Position) -> bool {
    (-MAX_WORLD_COORDINATE..=MAX_WORLD_COORDINATE).contains(&position.x)
        && (0..=MAX_WORLD_HEIGHT).contains(&position.y)
        && (-MAX_WORLD_COORDINATE..=MAX_WORLD_COORDINATE).contains(&position.z)
}

pub(super) fn inventory_count(resident: &Resident, resource: ResourceKind) -> u32 {
    resident.inventory.get(&resource).copied().unwrap_or(0)
}

pub(super) fn can_add_resource(resident: &Resident, resource: ResourceKind, amount: u32) -> bool {
    inventory_count(resident, resource)
        .checked_add(amount)
        .is_some_and(|total| total <= MAX_INVENTORY_PER_RESOURCE)
}

pub(super) fn materials_available(resident: &Resident, materials: &[ResourceAmount]) -> bool {
    if materials.is_empty() {
        return false;
    }
    let mut required = BTreeMap::<ResourceKind, u32>::new();
    for material in materials {
        if material.amount == 0 {
            return false;
        }
        let Some(total) = required
            .get(&material.resource)
            .copied()
            .unwrap_or(0)
            .checked_add(material.amount)
        else {
            return false;
        };
        required.insert(material.resource, total);
    }
    required
        .iter()
        .all(|(resource, amount)| inventory_count(resident, *resource) >= *amount)
}

pub(super) fn add_resource(resident: &mut Resident, resource: ResourceKind, amount: u32) {
    let entry = resident.inventory.entry(resource).or_default();
    *entry = entry.saturating_add(amount).min(MAX_INVENTORY_PER_RESOURCE);
}

pub(super) fn remove_resource(resident: &mut Resident, resource: ResourceKind, amount: u32) {
    if let Some(entry) = resident.inventory.get_mut(&resource) {
        *entry -= amount;
        if *entry == 0 {
            resident.inventory.remove(&resource);
        }
    }
}

pub(super) fn is_edible(resource: ResourceKind) -> bool {
    matches!(
        resource,
        ResourceKind::Berries | ResourceKind::Grain | ResourceKind::Meat | ResourceKind::Fish
    )
}

pub(super) fn gather_resource(kind: WorldObjectKind) -> Option<ResourceKind> {
    match kind {
        WorldObjectKind::Tree => Some(ResourceKind::Wood),
        WorldObjectKind::Rock => Some(ResourceKind::Stone),
        WorldObjectKind::BerryBush => Some(ResourceKind::Berries),
        WorldObjectKind::GrainPatch => Some(ResourceKind::Grain),
        WorldObjectKind::FiberPlant => Some(ResourceKind::Fiber),
        WorldObjectKind::ClayBank => Some(ResourceKind::Clay),
        _ => None,
    }
}

pub(super) fn harvest_resource(kind: WorldObjectKind) -> Option<ResourceKind> {
    match kind {
        WorldObjectKind::BerryBush => Some(ResourceKind::Berries),
        WorldObjectKind::GrainPatch => Some(ResourceKind::Grain),
        WorldObjectKind::Crop(resource) if is_edible(resource) => Some(resource),
        _ => None,
    }
}

pub(super) fn structure_cost(structure: StructureKind) -> &'static [ResourceAmount] {
    use ResourceKind::{Fiber, Stone, Wood};
    const LEAN_TO: &[ResourceAmount] = &[
        ResourceAmount {
            resource: Wood,
            amount: 5,
        },
        ResourceAmount {
            resource: Fiber,
            amount: 2,
        },
    ];
    const FIRE_PIT: &[ResourceAmount] = &[ResourceAmount {
        resource: Stone,
        amount: 4,
    }];
    const STORAGE: &[ResourceAmount] = &[ResourceAmount {
        resource: Wood,
        amount: 4,
    }];
    const WORKBENCH: &[ResourceAmount] = &[
        ResourceAmount {
            resource: Wood,
            amount: 6,
        },
        ResourceAmount {
            resource: Stone,
            amount: 2,
        },
    ];
    const GARDEN: &[ResourceAmount] = &[
        ResourceAmount {
            resource: Wood,
            amount: 2,
        },
        ResourceAmount {
            resource: Fiber,
            amount: 1,
        },
    ];
    const FENCE: &[ResourceAmount] = &[ResourceAmount {
        resource: Wood,
        amount: 3,
    }];
    const RAFT: &[ResourceAmount] = &[
        ResourceAmount {
            resource: Wood,
            amount: 8,
        },
        ResourceAmount {
            resource: Fiber,
            amount: 3,
        },
    ];
    match structure {
        StructureKind::LeanTo => LEAN_TO,
        StructureKind::FirePit => FIRE_PIT,
        StructureKind::StorageRack => STORAGE,
        StructureKind::Workbench => WORKBENCH,
        StructureKind::Garden => GARDEN,
        StructureKind::Fence => FENCE,
        StructureKind::Raft => RAFT,
    }
}

pub(super) fn recipe_cost(recipe: CraftRecipe) -> (&'static [ResourceAmount], ResourceKind) {
    use ResourceKind::{Basket, Fiber, Net, Rope, Spear, Stone, StoneAxe, Wood};
    const AXE: &[ResourceAmount] = &[
        ResourceAmount {
            resource: Wood,
            amount: 1,
        },
        ResourceAmount {
            resource: Stone,
            amount: 2,
        },
        ResourceAmount {
            resource: Fiber,
            amount: 1,
        },
    ];
    const SPEAR: &[ResourceAmount] = &[
        ResourceAmount {
            resource: Wood,
            amount: 2,
        },
        ResourceAmount {
            resource: Stone,
            amount: 1,
        },
    ];
    const ROPE: &[ResourceAmount] = &[ResourceAmount {
        resource: Fiber,
        amount: 3,
    }];
    const BASKET: &[ResourceAmount] = &[
        ResourceAmount {
            resource: Wood,
            amount: 1,
        },
        ResourceAmount {
            resource: Fiber,
            amount: 2,
        },
    ];
    const NET: &[ResourceAmount] = &[
        ResourceAmount {
            resource: Fiber,
            amount: 5,
        },
        ResourceAmount {
            resource: Rope,
            amount: 1,
        },
    ];
    match recipe {
        CraftRecipe::StoneAxe => (AXE, StoneAxe),
        CraftRecipe::Spear => (SPEAR, Spear),
        CraftRecipe::Rope => (ROPE, Rope),
        CraftRecipe::Basket => (BASKET, Basket),
        CraftRecipe::Net => (NET, Net),
    }
}

pub(super) fn parse_recipe(name: &str) -> Option<CraftRecipe> {
    match name
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
        .as_str()
    {
        "stone_axe" | "axe" => Some(CraftRecipe::StoneAxe),
        "spear" => Some(CraftRecipe::Spear),
        "rope" => Some(CraftRecipe::Rope),
        "basket" => Some(CraftRecipe::Basket),
        "net" | "fishing_net" => Some(CraftRecipe::Net),
        _ => None,
    }
}

pub(super) fn structure_name(structure: StructureKind) -> String {
    format!("{structure:?}").to_ascii_lowercase()
}
