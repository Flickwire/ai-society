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
    Query { questions: Vec<StateQuestion> },
    /// Submit a typed action after gathering enough context.
    Action { action: Action },
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

pub(super) fn normalize_questions(questions: &mut Vec<StateQuestion>) -> Vec<StateQuestion> {
    let mut seen = HashSet::new();
    questions
        .drain(..)
        .filter_map(|mut question| {
            normalize_question(&mut question);
            let key = format!("{question:?}");
            (seen.insert(key)).then_some(question)
        })
        .take(MAX_QUESTIONS_PER_BATCH)
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
