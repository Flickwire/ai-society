//! LLM action loop, output validation, and fail-closed inference behavior.

mod perception;
mod storyteller;

use society_domain::{Action, ActionProposal, WorldEvent};
use society_inference::InferenceProvider;
use society_sim::Village;
use society_storage::Store;

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
                let expected_version = village.version();
                let decision = perception::request_resident_action(provider, village, &resident)
                    .await
                    .map_err(|error| {
                        format!(
                            "{} inference failed ({error}); simulation stopped without deterministic fallback",
                            resident.name
                        )
                    })?;
                if !quiet && !decision.queries.is_empty() {
                    eprintln!(
                        "{} asked the simulator for: {}",
                        resident.name,
                        decision.queries.join(", ")
                    );
                }
                let proposal = ActionProposal {
                    resident: resident.id,
                    expected_version,
                    idempotency_key: format!("inference-{}-{}", resident.id.0, report.time.0),
                    action: decision.action,
                };
                match village.apply(proposal) {
                    WorldEvent::ActionCommitted { action, .. } => {
                        report
                            .actions
                            .push(format!("{}: {}", resident.name, action_name(&action)))
                    }
                    WorldEvent::ActionRejected { reason, .. } => {
                        return Err(format!(
                            "{} action rejected ({reason:?}); simulation stopped without deterministic fallback",
                            resident.name
                        )
                        .into());
                    }
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
