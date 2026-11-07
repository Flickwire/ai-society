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
    NaturalLanguage(String),
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

pub(super) fn normalize_questions(questions: &mut Vec<QueryInput>) -> Vec<StateQuestion> {
    let mut seen = HashSet::new();
    questions
        .drain(..)
        .filter_map(|input| {
            let mut question = match input {
                QueryInput::Structured(question) => question,
                QueryInput::NaturalLanguage(text) => natural_language_query(&text)?,
            };
            normalize_question(&mut question);
            let key = format!("{question:?}");
            (seen.insert(key)).then_some(question)
        })
        .take(MAX_QUESTIONS_PER_BATCH)
        .collect()
}

fn natural_language_query(text: &str) -> Option<StateQuestion> {
    let text = text.to_ascii_lowercase();
    if ["inventory", "carrying", "carried", "items"]
        .iter()
        .any(|term| text.contains(term))
    {
        Some(StateQuestion::Inventory)
    } else if ["note", "notes"].iter().any(|term| text.contains(term)) {
        Some(StateQuestion::Notes { limit: 8 })
    } else if ["memory", "memories", "remember", "recollect"]
        .iter()
        .any(|term| text.contains(term))
    {
        Some(StateQuestion::Memories {
            query: None,
            about: None,
            limit: 4,
        })
    } else if ["nearby", "surrounding", "environment", "objects", "see"]
        .iter()
        .any(|term| text.contains(term))
    {
        Some(StateQuestion::NearbyObjects {
            radius: 8,
            limit: 6,
        })
    } else if ["resident", "people", "person", "who is", "who are"]
        .iter()
        .any(|term| text.contains(term))
    {
        Some(StateQuestion::NearbyPeople {
            radius: 8,
            limit: 6,
        })
    } else if ["conversation", "talk", "said", "told"]
        .iter()
        .any(|term| text.contains(term))
    {
        Some(StateQuestion::Conversations {
            with: None,
            limit: 4,
        })
    } else if ["belief", "opinion", "attitude", "prefer"]
        .iter()
        .any(|term| text.contains(term))
    {
        Some(StateQuestion::Attitudes {
            topic: None,
            about: None,
            limit: 6,
        })
    } else if ["invention", "invent"]
        .iter()
        .any(|term| text.contains(term))
    {
        Some(StateQuestion::Inventions {
            query: None,
            limit: 4,
        })
    } else if ["capability", "proposal", "request"]
        .iter()
        .any(|term| text.contains(term))
    {
        Some(StateQuestion::OpenCapabilityRequests { limit: 4 })
    } else if ["place", "landmark", "where"]
        .iter()
        .any(|term| text.contains(term))
    {
        Some(StateQuestion::Places {
            query: None,
            limit: 4,
        })
    } else {
        None
    }
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
