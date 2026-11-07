//! LLM action loop, output validation, and deterministic fallback behavior.

mod perception;

use society_domain::{Action, ActionProposal, WorldEvent};
use society_inference::InferenceProvider;
use society_sim::{Resident, Village};
use society_storage::Store;

pub(super) async fn run_with_inference(
    village: &mut Village,
    store: &Store,
    hours: u64,
    quiet: bool,
    provider: &impl InferenceProvider,
) -> Result<Vec<society_sim::TickReport>, Box<dyn std::error::Error>> {
    let mut reports = Vec::with_capacity(usize::try_from(hours).unwrap_or(0));
    let mut provider_available = true;
    for _ in 0..hours {
        let mut report = village
            .advance_for_decisions()
            .map_err(|error| format!("simulation stopped: {error:?}"))?;
        let resident_ids: Vec<_> = village
            .residents()
            .iter()
            .map(|resident| resident.id)
            .collect();
        for resident_id in resident_ids {
            let resident = village
                .residents()
                .iter()
                .find(|resident| resident.id == resident_id)
                .cloned()
                .expect("resident IDs came from the current village");
            let expected_version = village.version();
            let action = if provider_available {
                match perception::request_resident_action(provider, village, &resident).await {
                    Ok(decision) => {
                        if !quiet && !decision.queries.is_empty() {
                            eprintln!(
                                "{} asked the simulator for: {}",
                                resident.name,
                                decision.queries.join(", ")
                            );
                        }
                        decision.action
                    }
                    Err(error) => {
                        if !quiet {
                            eprintln!(
                                "{} inference failed ({error}); using safe fallback for the rest of this run",
                                resident.name
                            );
                        }
                        provider_available = false;
                        fallback_action(village, &resident)
                    }
                }
            } else {
                fallback_action(village, &resident)
            };
            let proposal = ActionProposal {
                resident: resident.id,
                expected_version,
                idempotency_key: format!("inference-{}-{}", resident.id.0, report.time.0),
                action,
            };
            let mut event = village.apply(proposal);
            if matches!(event, WorldEvent::ActionRejected { .. }) {
                if !quiet {
                    eprintln!(
                        "{} proposed an invalid action; using safe fallback",
                        resident.name
                    );
                }
                let fallback = ActionProposal {
                    resident: resident.id,
                    expected_version: village.version(),
                    idempotency_key: format!("fallback-{}-{}", resident.id.0, report.time.0),
                    action: fallback_action(village, &resident),
                };
                event = village.apply(fallback);
            }
            if let WorldEvent::ActionCommitted { action, .. } = event {
                report
                    .actions
                    .push(format!("{}: {}", resident.name, action_name(&action)));
            }
        }
        store.save(village).await?;
        reports.push(report);
    }
    Ok(reports)
}

fn validate_agent_action(action: Action) -> Result<Action, Box<dyn std::error::Error>> {
    match action {
        Action::MoveTo { .. }
        | Action::Observe
        | Action::Speak { .. }
        | Action::Eat
        | Action::EatResource { .. }
        | Action::Drink { .. }
        | Action::Rest { .. }
        | Action::Gather { .. }
        | Action::Hunt { .. }
        | Action::Fish { .. }
        | Action::Build { .. }
        | Action::Destroy { .. }
        | Action::Dig { .. }
        | Action::Plant { .. }
        | Action::Harvest { .. }
        | Action::CraftItem { .. }
        | Action::ShareResource { .. }
        | Action::Play { .. }
        | Action::SetGoal { .. }
        | Action::Invent { .. }
        | Action::ProposeCapability { .. }
        | Action::VoteCapability { .. }
        | Action::Move { .. }
        | Action::Craft { .. } => Ok(action),
        Action::StartProject { .. }
        | Action::ContinueProject { .. }
        | Action::CancelProject { .. }
        | Action::OfferTrade { .. }
        | Action::AcceptTrade { .. }
        | Action::Forage { .. }
        | Action::Work { .. } => {
            Err("that action type is not implemented yet; request it as a capability".into())
        }
    }
}

fn fallback_action(village: &Village, resident: &Resident) -> Action {
    if resident.hunger >= 65
        && (resident.food > 0
            || resident.inventory.iter().any(|(item, amount)| {
                *amount > 0
                    && matches!(
                        item,
                        society_domain::ResourceKind::Berries
                            | society_domain::ResourceKind::Grain
                            | society_domain::ResourceKind::Meat
                            | society_domain::ResourceKind::Fish
                    )
            }))
    {
        Action::Eat
    } else if resident.thirst >= 60 {
        if let Some(water) = village.world_objects().iter().find(|object| {
            object.kind == society_sim::WorldObjectKind::WaterSource
                && resident.position.distance(object.position) <= 1
        }) {
            Action::Drink { source: water.id }
        } else if let Some(water) = village
            .world_objects()
            .iter()
            .filter(|object| object.kind == society_sim::WorldObjectKind::WaterSource)
            .min_by_key(|object| resident.position.distance(object.position))
        {
            Action::MoveTo {
                destination: step_toward(resident.position, water.position),
            }
        } else {
            Action::Observe
        }
    } else if resident.hunger >= 55 {
        if let Some(food) = village
            .world_objects()
            .iter()
            .filter(|object| {
                matches!(
                    object.kind,
                    society_sim::WorldObjectKind::BerryBush
                        | society_sim::WorldObjectKind::GrainPatch
                ) && object.quantity > 0
            })
            .min_by_key(|object| resident.position.distance(object.position))
        {
            if resident.position.distance(food.position) <= 1 {
                Action::Gather {
                    target: food.id,
                    amount: 1,
                }
            } else {
                Action::MoveTo {
                    destination: step_toward(resident.position, food.position),
                }
            }
        } else {
            Action::Observe
        }
    } else if resident.energy < 35 {
        Action::Rest { duration: 1 }
    } else {
        Action::Observe
    }
}

fn step_toward(
    current: society_domain::Position,
    target: society_domain::Position,
) -> society_domain::Position {
    let step = |from: i32, to: i32| from + (to - from).signum() * (to - from).abs().min(12);
    society_domain::Position::new(
        step(current.x, target.x),
        step(current.y, target.y),
        step(current.z, target.z),
    )
}

fn action_name(action: &Action) -> &'static str {
    match action {
        Action::Move { .. } => "moves",
        Action::Observe => "observes",
        Action::Speak { .. } => "speaks",
        Action::Eat => "eats",
        Action::Rest { .. } => "rests",
        Action::Forage { .. } => "forages",
        Action::Craft { .. } => "crafts",
        Action::Work { .. } => "works",
        Action::OfferTrade { .. } => "offers a trade",
        Action::AcceptTrade { .. } => "accepts a trade",
        Action::StartProject { .. } => "starts a project",
        Action::ContinueProject { .. } => "continues a project",
        Action::CancelProject { .. } => "cancels a project",
        Action::MoveTo { .. } => "moves through the wilderness",
        Action::EatResource { .. } => "eats",
        Action::Drink { .. } => "drinks",
        Action::Gather { .. } => "gathers",
        Action::Hunt { .. } => "hunts",
        Action::Fish { .. } => "fishes",
        Action::Build { .. } => "builds",
        Action::Destroy { .. } => "destroys",
        Action::Dig { .. } => "digs",
        Action::Plant { .. } => "plants",
        Action::Harvest { .. } => "harvests",
        Action::CraftItem { .. } => "crafts",
        Action::ShareResource { .. } => "shares resources",
        Action::Play { .. } => "plays",
        Action::SetGoal { .. } => "sets a goal",
        Action::Invent { .. } => "invents",
        Action::ProposeCapability { .. } => "requests a new capability",
        Action::VoteCapability { .. } => "votes on a capability",
    }
}
