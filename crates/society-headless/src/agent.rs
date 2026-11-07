//! LLM action loop, output validation, and fail-closed inference behavior.

mod perception;
mod storyteller;

use society_domain::{Action, ActionProposal, WorldEvent};
use society_inference::InferenceProvider;
use society_sim::{Resident, Village, WorldObjectKind};
use society_storage::Store;

const MAX_INVALID_ACTION_RETRIES: u8 = 2;

pub(super) struct RunResult {
    pub reports: Vec<society_sim::TickReport>,
    pub stories: Vec<(u64, String)>,
}

pub(super) async fn run(
    village: &mut Village,
    store: &Store,
    hours: u64,
    quiet: bool,
    use_inference: bool,
    use_storyteller: bool,
    storyteller_interval: u64,
    provider: &impl InferenceProvider,
) -> Result<RunResult, Box<dyn std::error::Error>> {
    let mut reports = Vec::with_capacity(usize::try_from(hours).unwrap_or(0));
    let mut storyteller_available = use_storyteller;
    let mut period_start = village.snapshot();
    let mut period_reports = Vec::new();
    let mut stories = Vec::new();
    let mut next_story_at = village.now().0.saturating_add(storyteller_interval.max(1));

    for completed_hours in 0..hours {
        let mut report = if use_inference {
            village
                .advance_for_decisions()
                .map_err(|error| format!("simulation stopped: {error:?}"))?
        } else {
            village
                .tick()
                .map_err(|error| format!("simulation stopped: {error:?}"))?
        };
        if use_inference {
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
                let mut correction = None;
                let mut query_trace = Vec::new();
                let mut cached_state = String::new();
                let mut cached_queries = Vec::new();
                let mut committed = false;
                for attempt in 0..=MAX_INVALID_ACTION_RETRIES {
                    let decision = match perception::request_resident_action(
                        provider,
                        village,
                        &resident,
                        correction.as_deref(),
                        &cached_state,
                        &cached_queries,
                    )
                    .await
                    {
                        Ok(decision) => decision,
                        Err(perception::DecisionError::Provider(error)) => {
                            return Err(format!(
                                "{} inference failed ({error}); simulation stopped without deterministic fallback",
                                resident.name
                            )
                            .into());
                        }
                        Err(perception::DecisionError::InvalidDecision(error))
                            if attempt < MAX_INVALID_ACTION_RETRIES =>
                        {
                            if !quiet {
                                eprintln!(
                                    "{} returned an invalid decision ({error}); asking the resident to reconsider",
                                    resident.name
                                );
                            }
                            correction = Some(format!(
                                "Your previous response could not be used: {error}. Review the required JSON schemas and choose a valid action; ask for state if needed."
                            ));
                            continue;
                        }
                        Err(perception::DecisionError::InvalidDecision(error)) => {
                            return Err(format!(
                                "{} returned an invalid decision after {} correction attempts ({error}); simulation stopped without deterministic fallback",
                                resident.name, MAX_INVALID_ACTION_RETRIES
                            )
                            .into());
                        }
                    };
                    for query in &decision.queries {
                        if !query_trace.contains(query) {
                            query_trace.push(query.clone());
                        }
                    }
                    cached_state = decision.requested_state.clone();
                    cached_queries = decision.queries.clone();
                    let proposal = ActionProposal {
                        resident: resident.id,
                        expected_version: village.version(),
                        idempotency_key: format!(
                            "inference-{}-{}-attempt-{attempt}",
                            resident.id.0, report.time.0
                        ),
                        action: decision.action,
                    };
                    let proposed_action_name = action_name(&proposal.action);
                    let proposed_action = proposal.action.clone();
                    match village.apply(proposal) {
                        WorldEvent::ActionCommitted { action, .. } => {
                            report.actions.push(format!(
                                "{}: {}",
                                resident.name,
                                action_name(&action)
                            ));
                            committed = true;
                            break;
                        }
                        WorldEvent::ActionRejected { reason, .. }
                            if attempt < MAX_INVALID_ACTION_RETRIES =>
                        {
                            if !quiet {
                                eprintln!(
                                    "{} proposed an invalid {proposed_action_name} action ({reason:?}); asking the resident to reconsider",
                                    resident.name
                                );
                            }
                            correction = Some(rejection_feedback(
                                village,
                                &resident,
                                &proposed_action,
                                proposed_action_name,
                                reason,
                            ));
                        }
                        WorldEvent::ActionRejected { reason, .. } => {
                            return Err(format!(
                                "{} proposed an invalid {proposed_action_name} action ({reason:?}) after {} correction attempts; simulation stopped without deterministic fallback",
                                resident.name, MAX_INVALID_ACTION_RETRIES
                            )
                            .into());
                        }
                    }
                }
                if !committed {
                    return Err(format!(
                        "{} did not produce a valid action after correction attempts; simulation stopped without deterministic fallback",
                        resident.name
                    )
                    .into());
                }
                if !quiet && !query_trace.is_empty() {
                    eprintln!(
                        "{} asked the simulator for: {}",
                        resident.name,
                        query_trace.join(", ")
                    );
                }
            }
        }
        store.save(village).await?;
        period_reports.push(report.clone());
        reports.push(report);

        let final_hour = completed_hours + 1 == hours;
        if storyteller_available && (village.now().0 >= next_story_at || final_hour) {
            match storyteller::summarize_period(
                provider,
                &period_reports,
                &period_start,
                &village.snapshot(),
            )
            .await
            {
                Ok(summary) => {
                    stories.push((village.now().0, summary));
                }
                Err(error) => {
                    eprintln!("Storyteller failed ({error}); disabling narration for this run.");
                    storyteller_available = false;
                }
            }
            period_reports.clear();
            period_start = village.snapshot();
            next_story_at = village.now().0.saturating_add(storyteller_interval.max(1));
        }
    }
    Ok(RunResult { reports, stories })
}

fn rejection_feedback(
    village: &Village,
    resident: &Resident,
    action: &Action,
    action_name: &str,
    reason: society_domain::RejectionReason,
) -> String {
    let general = format!(
        "The simulator rejected your {action_name} action ({reason:?}); it made no world change. Review the action's preconditions, request relevant state if needed, and choose a different valid action. Do not repeat the rejected action."
    );
    match action {
        Action::Gather { target, amount } => {
            let Some(object) = village
                .world_objects()
                .iter()
                .find(|object| object.id == *target)
            else {
                return format!(
                    "{general} Object ID {} does not exist; ask for nearby_objects and use a listed numeric ID.",
                    target.0
                );
            };
            let distance = resident.position.distance(object.position);
            if distance > 1 {
                format!(
                    "{general} Object #{} is {} units away at ({},{},{}); gathering requires distance 1 or less. Move closer first, then gather.",
                    target.0, distance, object.position.x, object.position.y, object.position.z
                )
            } else if !matches!(
                object.kind,
                WorldObjectKind::Tree
                    | WorldObjectKind::Rock
                    | WorldObjectKind::BerryBush
                    | WorldObjectKind::GrainPatch
                    | WorldObjectKind::FiberPlant
                    | WorldObjectKind::ClayBank
            ) {
                format!(
                    "{general} Object #{} ({:?}) cannot be gathered; choose a gatherable natural resource.",
                    target.0, object.kind
                )
            } else if *amount > object.quantity {
                format!(
                    "{general} Object #{} has stock {}, but you requested {}; request a smaller amount.",
                    target.0, object.quantity, amount
                )
            } else {
                format!(
                    "{general} Object #{} is in reach with stock {}; check your inventory capacity or choose another action.",
                    target.0, object.quantity
                )
            }
        }
        _ => general,
    }
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
        | Action::KeepNote { .. }
        | Action::UpdateNote { .. }
        | Action::DeleteNote { .. }
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
        Action::KeepNote { .. } => "keeps a note",
        Action::UpdateNote { .. } => "updates a note",
        Action::DeleteNote { .. } => "deletes a note",
        Action::Invent { .. } => "invents",
        Action::ProposeCapability { .. } => "requests a new capability",
        Action::VoteCapability { .. } => "votes on a capability",
    }
}
