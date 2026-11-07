//! Bounded read-only state queries used before the resident chooses an action.

mod queries;
mod schema;

use queries::{
    answer_questions, append_query_results, supported_action_schema, supported_query_schema,
};
use schema::{InferenceTurn, MAX_QUERY_ROUNDS, normalize_questions};
use society_domain::Action;
use society_inference::{ChatMessage, ChatRequest, InferenceProvider};
use society_sim::{Resident, Village};

pub(super) struct ResidentDecision {
    pub action: Action,
    /// Query summaries only; never include retrieved private state in logs.
    pub queries: Vec<String>,
}

pub(super) async fn request_resident_action(
    provider: &impl InferenceProvider,
    village: &Village,
    resident: &Resident,
) -> Result<ResidentDecision, Box<dyn std::error::Error>> {
    let baseline = resident_baseline(village, resident);
    let first = provider
        .chat(ChatRequest {
            messages: vec![
                ChatMessage::new(
                    "system",
                    format!(
                        "Before deciding, ask the simulator only for facts you need. Return {{\"type\":\"query\",\"questions\":[...]}}. Do not choose an action yet. Query objects use these exact schemas: {}. You may ask an empty list. Queries are read-only and bounded; memories and attitudes are private to you.",
                        supported_query_schema()
                    ),
                ),
                ChatMessage::new("user", baseline.clone()),
            ],
            max_tokens: 512,
            temperature: 0.1,
            response_format: None,
            reasoning_effort: Some("low".to_owned()),
        })
        .await?;
    let mut questions = match parse_turn(&first.content)? {
        InferenceTurn::Query { questions } => questions,
        InferenceTurn::Action { .. } => {
            return Err("resident must query state before choosing an action".into());
        }
    };
    let mut context = String::new();
    let mut query_trace = Vec::new();
    let mut follow_up_rounds = 0;
    let normalized = normalize_questions(&mut questions);
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
        .await?;
        match parse_turn(&reply.content)? {
            InferenceTurn::Action { action } => {
                return Ok(ResidentDecision {
                    action: super::validate_agent_action(action)?,
                    queries: query_trace,
                });
            }
            InferenceTurn::Query { mut questions } if follow_up_rounds < MAX_QUERY_ROUNDS => {
                follow_up_rounds += 1;
                // No additional state is added to the prompt until requested.
                let normalized = normalize_questions(&mut questions);
                record_query_summaries(&mut query_trace, &normalized);
                append_query_results(
                    &mut context,
                    answer_questions(village, resident, normalized),
                );
            }
            InferenceTurn::Query { .. } => break,
        }
    }

    let final_reply = ask_for_action(provider, &baseline, &context).await?;
    match parse_turn(&final_reply.content)? {
        InferenceTurn::Action { action } => Ok(ResidentDecision {
            action: super::validate_agent_action(action)?,
            queries: query_trace,
        }),
        InferenceTurn::Query { .. } => {
            Err("resident continued querying after the query-round limit".into())
        }
    }
}

fn record_query_summaries(trace: &mut Vec<String>, questions: &[schema::StateQuestion]) {
    trace.extend(questions.iter().map(|question| format!("{question:?}")));
}

async fn ask_next_step(
    provider: &impl InferenceProvider,
    baseline: &str,
    context: &str,
    may_query: bool,
) -> Result<society_inference::ChatResponse, Box<dyn std::error::Error>> {
    let instruction = if may_query {
        "You have received only the facts you requested. Either request another necessary fact with {\"type\":\"query\",\"questions\":[...]}, or choose one action with {\"type\":\"action\",\"action\":{...}}. Ask only questions that could change your decision."
    } else {
        "You have received the facts you requested. Choose one action now with {\"type\":\"action\",\"action\":{...}}. Do not request more context."
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
            max_tokens: 768,
            temperature: 0.2,
            response_format: None,
            reasoning_effort: Some("low".to_owned()),
        })
        .await
        .map_err(Into::into)
}

async fn ask_for_action(
    provider: &impl InferenceProvider,
    baseline: &str,
    context: &str,
) -> Result<society_inference::ChatResponse, Box<dyn std::error::Error>> {
    provider
        .chat(ChatRequest {
            messages: vec![
                ChatMessage::new(
                    "system",
                    format!(
                        "Choose one immediate action. You cannot ask more questions. Return {{\"type\":\"action\",\"action\":<typed action>}} as JSON only. Supported actions: {}. Use exact resource, structure, recipe, and resident IDs from requested state. If unsure, observe.",
                        supported_action_schema()
                    ),
                ),
                ChatMessage::new("user", format!("{baseline}\nRequested state:\n{context}")),
            ],
            max_tokens: 768,
            temperature: 0.2,
            response_format: None,
            reasoning_effort: Some("low".to_owned()),
        })
        .await
        .map_err(Into::into)
}

fn resident_baseline(village: &Village, resident: &Resident) -> String {
    let goal = if resident.goal.is_empty() {
        "(none chosen)"
    } else {
        &resident.goal
    };
    format!(
        "Hour {}. You are {} id {} at ({},{},{}). Goal: {}. Needs: hunger {}, thirst {}, energy {}, health {}, morale {}. Body: height {}cm, mass {}kg, fitness {}, strength {}. Intelligence {} / 100. Personality: openness {}, conscientiousness {}, extraversion {}, agreeableness {}, emotional stability {}.",
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
    Ok(serde_json::from_str(json)?)
}
