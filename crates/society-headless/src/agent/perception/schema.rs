//! Typed allowlist and size limits for resident state queries.

use serde::Deserialize;
use society_domain::Action;
use std::collections::HashSet;

const MAX_QUESTIONS_PER_BATCH: usize = 6;
pub(super) const MAX_QUERY_ROUNDS: usize = 2;
const MAX_QUERY_TEXT_LENGTH: usize = 80;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum InferenceTurn {
    /// Ask the simulator for selected context before acting.
    Query { questions: Vec<QueryInput> },
    /// Submit a typed action after gathering enough context.
    Action { action: Action },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum QueryInput {
    Structured(StateQuestion),
    /// Preserve malformed query input so the agent can be prompted to correct it.
    Other(serde_json::Value),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum StateQuestion {
    /// Inspect carried resources and crafted goods.
    Inventory,
    /// Inspect natural or built objects near the resident.
    NearbyObjects {
        #[serde(default = "default_radius")]
        radius: u8,
        #[serde(default = "default_object_limit")]
        limit: u8,
    },
    /// Recall personal memories by topic or person.
    Memories {
        query: Option<String>,
        about: Option<u64>,
        #[serde(default = "default_memory_limit")]
        limit: u8,
    },
    /// Review durable notes the resident chose to keep.
    Notes {
        #[serde(default = "default_note_limit")]
        limit: u8,
    },
    /// Inspect private beliefs, opinions, preferences, or impressions.
    Attitudes {
        topic: Option<String>,
        about: Option<u64>,
        #[serde(default = "default_attitude_limit")]
        limit: u8,
    },
    /// Identify nearby residents and their current public condition.
    NearbyPeople {
        #[serde(default = "default_people_radius")]
        radius: u8,
        #[serde(default = "default_people_limit")]
        limit: u8,
    },
    /// Recall conversations in which this resident participated.
    Conversations {
        with: Option<u64>,
        #[serde(default = "default_conversation_limit")]
        limit: u8,
    },
    /// Review pending capability requests and current vote counts.
    OpenCapabilityRequests {
        #[serde(default = "default_request_limit")]
        limit: u8,
    },
    /// Recall recorded inventions.
    Inventions {
        query: Option<String>,
        #[serde(default = "default_invention_limit")]
        limit: u8,
    },
    /// Find named landmarks, optionally by a text match.
    Places {
        query: Option<String>,
        #[serde(default = "default_place_limit")]
        limit: u8,
    },
}

fn default_radius() -> u8 {
    8
}
fn default_object_limit() -> u8 {
    6
}
fn default_memory_limit() -> u8 {
    4
}
fn default_note_limit() -> u8 {
    8
}
fn default_attitude_limit() -> u8 {
    6
}
fn default_people_radius() -> u8 {
    8
}
fn default_people_limit() -> u8 {
    6
}
fn default_conversation_limit() -> u8 {
    4
}
fn default_request_limit() -> u8 {
    4
}
fn default_invention_limit() -> u8 {
    4
}
fn default_place_limit() -> u8 {
    4
}

pub(super) fn normalize_questions(
    questions: &mut Vec<QueryInput>,
) -> Result<Vec<StateQuestion>, String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for input in questions.drain(..) {
        let mut question = match input {
            QueryInput::Structured(question) => question,
            QueryInput::Other(value) => {
                let _ = value;
                return Err("the simulator does not understand unstructured or natural-language queries; use a structured query object with a supported kind".to_owned());
            }
        };
        normalize_question(&mut question);
        if seen.insert(format!("{question:?}")) {
            normalized.push(question);
            if normalized.len() >= MAX_QUESTIONS_PER_BATCH {
                break;
            }
        }
    }
    Ok(normalized)
}

pub(super) fn exclude_previous_questions(
    questions: Vec<StateQuestion>,
    asked: &mut HashSet<String>,
) -> Vec<StateQuestion> {
    questions
        .into_iter()
        .filter(|question| asked.insert(format!("{question:?}")))
        .collect()
}

fn normalize_question(question: &mut StateQuestion) {
    let limit = |value: &mut u8, max: u8| *value = (*value).clamp(1, max);
    let text = |query: &mut Option<String>| {
        if let Some(value) = query {
            if value.chars().count() > MAX_QUERY_TEXT_LENGTH {
                *value = value.chars().take(MAX_QUERY_TEXT_LENGTH).collect();
            }
        }
    };
    match question {
        StateQuestion::Inventory => {}
        StateQuestion::NearbyObjects {
            radius,
            limit: count,
        } => {
            limit(radius, 16);
            limit(count, 8);
        }
        StateQuestion::Memories {
            query,
            about: _,
            limit: count,
        } => {
            text(query);
            limit(count, 6);
        }
        StateQuestion::Notes { limit: count } => limit(count, 12),
        StateQuestion::Attitudes {
            topic,
            about: _,
            limit: count,
        } => {
            text(topic);
            limit(count, 8);
        }
        StateQuestion::NearbyPeople {
            radius,
            limit: count,
        } => {
            limit(radius, 16);
            limit(count, 8);
        }
        StateQuestion::Conversations {
            with: _,
            limit: count,
        } => limit(count, 6),
        StateQuestion::OpenCapabilityRequests { limit: count } => limit(count, 6),
        StateQuestion::Inventions {
            query,
            limit: count,
        } => {
            text(query);
            limit(count, 6);
        }
        StateQuestion::Places {
            query,
            limit: count,
        } => {
            text(query);
            limit(count, 6);
        }
    }
}
