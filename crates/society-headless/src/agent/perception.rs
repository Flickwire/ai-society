//! Bounded read-only state queries used before the resident chooses an action.

mod queries;
mod schema;

use queries::{
    answer_questions, append_query_results, format_objects, supported_action_schema,
    supported_query_schema,
};
use schema::{
    InferenceTurn, MAX_QUERY_ROUNDS, StateQuestion, exclude_previous_questions, normalize_questions,
};
use society_domain::Action;
use society_inference::{ChatMessage, ChatRequest, InferenceProvider};
use society_sim::{Resident, Village};
use std::collections::HashSet;

pub(super) struct ResidentDecision {
    pub action: Action,
    /// Query summaries only; never include retrieved private state in logs.
    pub queries: Vec<String>,
    /// State retrieved for this decision, reused if the model must correct an
    /// invalid action in the same unchanged world state.
    pub requested_state: String,
}

pub(super) enum DecisionError {
    Provider(String),
    InvalidDecision(String),
}

pub(super) async fn request_resident_action(
    provider: &impl InferenceProvider,
    village: &Village,
    resident: &Resident,
    correction: Option<&str>,
    previous_state: &str,
    previous_queries: &[String],
) -> Result<ResidentDecision, DecisionError> {
    let baseline = match correction {
        Some(correction) => format!(
            "{}\nPrevious decision feedback: {correction}",
            resident_baseline(village, resident)
        ),
        None => resident_baseline(village, resident),
    };
    let mut context = previous_state.to_owned();
    let mut query_trace = previous_queries.to_vec();
    let initial_user = if context.is_empty() {
        baseline.clone()
    } else {
        format!("{baseline}\nPreviously requested simulator state:\n{context}")
    };
    let first = provider
        .chat(ChatRequest {
            messages: vec![
                ChatMessage::new(
                    "system",
                    format!(
                        "Before deciding, ask the simulator only for facts you need. It does not understand unstructured or natural-language queries; questions must be structured JSON objects with a supported kind, never strings or prose. Return JSON exactly shaped like {{\"type\":\"query\",\"questions\":[{{\"kind\":\"inventory\"}}]}}. Every questions entry MUST match one of these exact schemas: {}. Do not choose an action yet. You may ask an empty list. Queries are read-only and bounded; memories and attitudes are private to you. The initial resident state already includes nearby objects within radius 8, their numeric IDs, distances, and stock; do not request that information again unless you need a different radius. Previously requested state from a rejected attempt is also included in the user message; do not repeat those queries.",
                        supported_query_schema()
                    ),
                ),
                ChatMessage::new("user", initial_user),
            ],
            // The provider adaptively retries responses stopped by its token
            // limit, up to a bounded ceiling.
            max_tokens: 2_048,
            temperature: 0.1,
            response_format: None,
            reasoning_effort: Some("none".to_owned()),
        })
        .await
        .map_err(|error| DecisionError::Provider(error.to_string()))?;
    let mut questions = match parse_turn(&first.content)
        .map_err(|error| DecisionError::InvalidDecision(error.to_string()))?
    {
        InferenceTurn::Query { questions } => questions,
        InferenceTurn::Action { .. } => {
            return Err(DecisionError::InvalidDecision(
                "resident must query state before choosing an action".to_owned(),
            ));
        }
    };
    let mut asked_questions = HashSet::new();
    asked_questions.extend(previous_queries.iter().cloned());
    asked_questions.insert(format!(
        "{:?}",
        StateQuestion::NearbyObjects {
            radius: 8,
            limit: 6
        }
    ));
    let mut follow_up_rounds = 0;
    let normalized = normalize_questions(&mut questions).map_err(DecisionError::InvalidDecision)?;
    let normalized = exclude_previous_questions(normalized, &mut asked_questions);
    record_query_summaries(&mut query_trace, &normalized);
    append_query_results(
        &mut context,
        answer_questions(village, resident, normalized),
    );

    loop {
        let reply = ask_next_step(
            provider,
            &baseline,
            &context,
            follow_up_rounds < MAX_QUERY_ROUNDS,
        )
        .await
        .map_err(|error| DecisionError::Provider(error.to_string()))?;
        match parse_turn(&reply.content)
            .map_err(|error| DecisionError::InvalidDecision(error.to_string()))?
        {
            InferenceTurn::Action {
                action: Action::Observe,
            } => break,
            InferenceTurn::Action { action } => {
                return Ok(ResidentDecision {
                    action: super::validate_agent_action(action)
                        .map_err(|error| DecisionError::InvalidDecision(error.to_string()))?,
                    queries: query_trace,
                    requested_state: context,
                });
            }
            InferenceTurn::Query { mut questions } if follow_up_rounds < MAX_QUERY_ROUNDS => {
                follow_up_rounds += 1;
                // No additional state is added to the prompt until requested.
                let normalized =
                    normalize_questions(&mut questions).map_err(DecisionError::InvalidDecision)?;
                let normalized = exclude_previous_questions(normalized, &mut asked_questions);
                if normalized.is_empty() {
                    if follow_up_rounds < MAX_QUERY_ROUNDS {
                        append_query_results(
                            &mut context,
                        vec!["The simulator does not understand unstructured or natural-language queries; use a structured query object with a supported kind. That query was already answered. Ask for different state or choose an action.".to_owned()],
                        );
                        continue;
                    }
                    break;
                }
                record_query_summaries(&mut query_trace, &normalized);
                append_query_results(
                    &mut context,
                    answer_questions(village, resident, normalized),
                );
            }
            InferenceTurn::Query { .. } => break,
        }
    }

    let final_reply = ask_for_action(provider, &baseline, &context)
        .await
        .map_err(|error| DecisionError::Provider(error.to_string()))?;
    match parse_turn(&final_reply.content)
        .map_err(|error| DecisionError::InvalidDecision(error.to_string()))?
    {
        InferenceTurn::Action {
            action: Action::Observe,
        } => Err(DecisionError::InvalidDecision(
            "resident chose observe after being asked for a consequential action".to_owned(),
        )),
        InferenceTurn::Action { action } => Ok(ResidentDecision {
            action: super::validate_agent_action(action)
                .map_err(|error| DecisionError::InvalidDecision(error.to_string()))?,
            queries: query_trace,
            requested_state: context,
        }),
        InferenceTurn::Query { .. } => Err(DecisionError::InvalidDecision(
            "resident continued querying after the query-round limit".to_owned(),
        )),
    }
}

fn record_query_summaries(trace: &mut Vec<String>, questions: &[schema::StateQuestion]) {
    for summary in questions.iter().map(|question| format!("{question:?}")) {
        if !trace.contains(&summary) {
            trace.push(summary);
        }
    }
}

async fn ask_next_step(
    provider: &impl InferenceProvider,
    baseline: &str,
    context: &str,
    may_query: bool,
) -> Result<society_inference::ChatResponse, society_inference::InferenceError> {
    let instruction = if may_query {
        "You have received only the facts you requested. The simulator does not understand unstructured or natural-language queries; questions must be structured JSON objects with a supported kind, never strings or prose. Either request another necessary fact with {\"type\":\"query\",\"questions\":[...]}, or choose one consequential action. Do not choose observe: it neither reveals information nor changes the world; use state queries to learn. The outer type must be query or action; for actions, the nested action object must also have its own type field, such as {\"type\":\"gather\",\"target\":1,\"amount\":1}. If an action needs an object or resident ID, ask for nearby_objects or nearby_people first, then use the numeric ID exactly as returned; never guess IDs. Gathering, drinking, hunting, fishing, harvesting, and destroying require the target to be within one unit; check distances and move closer before acting when needed. Do not repeat questions already answered. Ask only questions that could change your decision."
    } else {
        "You have received the facts you requested. Choose one consequential action now with {\"type\":\"action\",\"action\":{...}}. Do not choose observe, and do not request more context."
    };
    provider
        .chat(ChatRequest {
            messages: vec![
                ChatMessage::new(
                    "system",
                    format!(
                        "{instruction} Supported actions: {}. {}Return JSON only. The simulator validates all actions.",
                        supported_action_schema(),
                        if may_query {
                            format!("Available query objects: {}. ", supported_query_schema())
                        } else {
                            String::new()
                        }
                    ),
                ),
                ChatMessage::new("user", format!("{baseline}\nRequested state:\n{context}")),
            ],
            max_tokens: 2_048,
            temperature: 0.2,
            response_format: None,
            reasoning_effort: Some("none".to_owned()),
        })
        .await
}

async fn ask_for_action(
    provider: &impl InferenceProvider,
    baseline: &str,
    context: &str,
) -> Result<society_inference::ChatResponse, society_inference::InferenceError> {
    provider
        .chat(ChatRequest {
            messages: vec![
                ChatMessage::new(
                    "system",
                    format!(
                        "Choose one immediate consequential action. You cannot ask more questions. Return JSON with both levels typed, for example {{\"type\":\"action\",\"action\":{{\"type\":\"move_to\",\"destination\":{{\"x\":1,\"y\":0,\"z\":1}}}}}}. The nested action object must have its own type field naming the action variant. Do not choose observe: it neither reveals information nor changes the world. Supported actions: {}. If an action refers to an object or resident, use its numeric ID exactly as shown in requested state; never return a descriptive name in an ID field. If you lack information for a targeted action, choose a different consequential action such as setting a self-chosen goal, moving, resting, speaking, or playing.",
                        supported_action_schema()
                    ),
                ),
                ChatMessage::new("user", format!("{baseline}\nRequested state:\n{context}")),
            ],
            max_tokens: 2_048,
            temperature: 0.2,
            response_format: None,
            reasoning_effort: Some("none".to_owned()),
        })
        .await
}

fn resident_baseline(village: &Village, resident: &Resident) -> String {
    let goal = if resident.goal.is_empty() {
        "(none chosen)"
    } else {
        &resident.goal
    };
    let nearby_objects = format_objects(village, resident, 8, 8);
    format!(
        "Hour {}. You are {} id {} at ({},{},{}). Goal: {}. Needs: hunger {}, thirst {}, energy {}, health {}, morale {}. Body: height {}cm, mass {}kg, fitness {}, strength {}. Intelligence {} / 100. Personality: openness {}, conscientiousness {}, extraversion {}, agreeableness {}, emotional stability {}. Nearby objects within radius 8 (IDs, distances, and stock): {}. Interaction radius is 1 unit for gathering, drinking, hunting, fishing, harvesting, destroying, building, digging, and planting; speaking works within 4 and sharing within 2. MoveTo can travel at most 12 units.",
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
        nearby_objects,
    )
}

fn parse_turn(content: &str) -> Result<InferenceTurn, Box<dyn std::error::Error>> {
    let json = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    if json.is_empty() {
        return Err("provider returned an empty query/action response".into());
    }
    let mut value: serde_json::Value = serde_json::from_str(json)?;
    if let Some(object) = value.as_object_mut()
        && !object.contains_key("type")
    {
        if object.contains_key("questions") {
            object.insert(
                "type".to_owned(),
                serde_json::Value::String("query".to_owned()),
            );
        } else if object.contains_key("action") {
            object.insert(
                "type".to_owned(),
                serde_json::Value::String("action".to_owned()),
            );
        }
    }
    if value.get("type").and_then(serde_json::Value::as_str) == Some("action")
        && let Some(action) = value
            .get_mut("action")
            .and_then(serde_json::Value::as_object_mut)
        && !action.contains_key("type")
        && let Some(action_type) = ["name", "kind", "action"]
            .iter()
            .find_map(|key| action.get(*key).and_then(serde_json::Value::as_str))
    {
        action.insert(
            "type".to_owned(),
            serde_json::Value::String(action_type.to_ascii_lowercase()),
        );
    }
    if let Some(action) = value
        .get_mut("action")
        .and_then(serde_json::Value::as_object_mut)
    {
        for field in ["target", "source"] {
            if let Some(serde_json::Value::String(label)) = action.get(field)
                && let Some(id) = explicit_display_id(label)
            {
                action.insert(field.to_owned(), serde_json::Value::Number(id.into()));
            }
        }
    }
    let fields = value.as_object().map_or_else(
        || "non-object JSON".to_owned(),
        |object| {
            let root_type = object
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("missing");
            let action_shape = object
                .get("action")
                .map(|action| match action {
                    serde_json::Value::Object(action) => {
                        format!(
                            "action fields: {}",
                            action.keys().cloned().collect::<Vec<_>>().join(", ")
                        )
                    }
                    serde_json::Value::String(_) => "action is a string".to_owned(),
                    _ => "action has another JSON shape".to_owned(),
                })
                .unwrap_or_default();
            format!("root type: {root_type}; {action_shape}")
        },
    );
    serde_json::from_value(value).map_err(|error| {
        format!("could not parse query/action JSON ({error}); response fields: {fields}").into()
    })
}

fn explicit_display_id(label: &str) -> Option<u64> {
    let label = label.trim();
    if let Ok(id) = label.parse() {
        return Some(id);
    }
    let digits = label
        .split_once('#')?
        .1
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    digits.parse().ok()
}
