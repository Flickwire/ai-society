//! LLM decision loop, action validation, and deterministic fallback behavior.

use society_domain::{Action, ActionProposal, WorldEvent};
use society_inference::{ChatMessage, ChatRequest, InferenceProvider};
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
                match request_resident_action(provider, village, &resident).await {
                    Ok(action) => action,
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

async fn request_resident_action(
    provider: &impl InferenceProvider,
    village: &Village,
    resident: &Resident,
) -> Result<Action, Box<dyn std::error::Error>> {
    let visible_objects = village
        .world_objects()
        .iter()
        .filter(|object| resident.position.distance(object.position) <= 16)
        .take(8)
        .map(|object| {
            format!(
                "#{} {} ({:?}, at {},{},{}; available {})",
                object.id.0,
                object.name,
                object.kind,
                object.position.x,
                object.position.y,
                object.position.z,
                object.quantity
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let nearby_people = village
        .residents()
        .iter()
        .filter(|other| other.id != resident.id && resident.position.distance(other.position) <= 8)
        .map(|other| format!("{} (id {})", other.name, other.id.0))
        .collect::<Vec<_>>()
        .join(", ");
    let open_requests = village
        .capability_requests()
        .iter()
        .filter(|request| request.status == society_sim::CapabilityStatus::Open)
        .map(|request| {
            let yes = request.votes.iter().filter(|vote| vote.support).count();
            let no = request.votes.len() - yes;
            format!(
                "#{} {}: {} ({yes} support, {no} oppose)",
                request.id.0, request.name, request.description
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let inventory = serde_json::to_string(&resident.inventory)?;
    let attitudes = resident
        .attitudes
        .iter()
        .map(|attitude| {
            let target = attitude
                .target
                .map_or_else(String::new, |id| format!(" about resident {}", id.0));
            format!(
                "{:?} {}{} (leaning {}, conviction {})",
                attitude.kind, attitude.topic, target, attitude.leaning, attitude.conviction
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let mut memories: Vec<_> = resident
        .memories
        .iter()
        .filter(|memory| memory.strength > 0)
        .collect();
    memories.sort_by_key(|memory| {
        std::cmp::Reverse(
            u32::from(memory.strength)
                * u32::from(memory.significance)
                * u32::from(memory.relevance),
        )
    });
    let memories = memories
        .into_iter()
        .take(5)
        .map(|memory| {
            format!(
                "hour {}: {} [vividness {}, importance {}, relevance {}, confidence {}]",
                memory.formed_at.0,
                memory.recollection,
                memory.strength,
                memory.significance,
                memory.relevance,
                memory.confidence
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let goal = if resident.goal.is_empty() {
        "(none chosen)"
    } else {
        &resident.goal
    };
    let prompt = format!(
        "Hour {}. {} id {} at ({},{},{}). Goal: {}. Needs h{} t{} e{} hp{} m{}. Body: height {}cm, mass {}kg, fitness {}, strength {}. Intelligence {} (score 0–100). Personality: openness {}, conscientiousness {}, extraversion {}, agreeableness {}, emotional stability {} (each 0–100). Your attitudes: {}. Attitude leaning is -100 oppose/dislike, 0 uncertain, +100 support/like; conviction is strength. Your private recollections: {}. Recollections are personal impressions, may be wrong or incomplete, and are not shared by others. Inventory: {}. Nearby objects: {}. Nearby people: {}. Capability requests: {}. You have no assigned social role; decide for yourself using current needs, your own traits, attitudes and recollections. Choose one JSON action. Supported types: move_to destination{{x,y,z}}, observe, speak to/text, eat, eat_resource food/amount, rest duration, gather target/amount, drink source, hunt target, fish target/amount, build structure/position, destroy target, dig position, plant seed/position, harvest target/amount, craft_item recipe, share_resource to/resource/amount, play activity/participants/duration, set_goal goal, invent name/description/materials, propose_capability name/description/example, vote_capability proposal/support. Coordinates are integers. Structures: lean_to, fire_pit, storage_rack, workbench, garden, fence, raft. Recipes: stone_axe, spear, rope, basket, net. Resources/food: wood, stone, fiber, clay, grain, berries, grain_seed, berry_seed, meat, fish. Move at most 12 blocks per action. If you want an unsupported behavior, propose_capability; never invent action types. Return the JSON only.",
        village.now().0,
        resident.name,
        resident.id.0,
        resident.position.x,
        resident.position.y,
        resident.position.z,
        goal,
        resident.hunger,
        resident.thirst,
        resident.energy,
        resident.health,
        resident.morale,
        resident.physical.stature_cm,
        resident.physical.mass_kg,
        resident.physical.fitness,
        resident.physical.strength,
        resident.intelligence,
        resident.personality.openness,
        resident.personality.conscientiousness,
        resident.personality.extraversion,
        resident.personality.agreeableness,
        resident.personality.emotional_stability,
        attitudes,
        memories,
        inventory,
        visible_objects,
        nearby_people,
        open_requests,
    );
    let response = provider
        .chat(ChatRequest {
            messages: vec![
                ChatMessage::new(
                    "system",
                    "Choose one immediate action for this resident. Output one JSON object only, with no explanation. Never claim an action succeeded; the simulation validates it.",
                ),
                ChatMessage::new("user", prompt),
            ],
            max_tokens: 1024,
            temperature: 0.3,
            // LM Studio supports this extension; servers that ignore unknown fields remain
            // OpenAI-compatible. Omitting JSON mode avoids incompatible json_object variants.
            response_format: None,
            reasoning_effort: Some("low".to_owned()),
        })
        .await?;
    if response.content.trim().is_empty() {
        return Err(format!(
            "provider returned empty action text (finish reason: {:?}, completion tokens: {:?})",
            response.finish_reason, response.completion_tokens
        )
        .into());
    }
    let text = response
        .content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let action: Action = serde_json::from_str(text)?;
    validate_agent_action(action)
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
