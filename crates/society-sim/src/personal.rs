//! Private resident attributes, attitudes, and fallible recollections.

use serde::{Deserialize, Serialize};
use society_domain::{ResidentId, VirtualTime};

/// Observable physical characteristics that can change through life experience.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PhysicalProfile {
    /// Height in centimetres. It is stable in the current age-free simulation.
    pub stature_cm: u16,
    /// Body mass in kilograms; prolonged deprivation can reduce it.
    pub mass_kg: u16,
    /// General conditioning from 0 (frail) to 100 (vigorous).
    pub fitness: u8,
    /// Functional strength from 0 to 100.
    pub strength: u8,
    /// Progress toward conditioning from repeated physical effort.
    #[serde(default)]
    pub conditioning_experience: u8,
}

impl Default for PhysicalProfile {
    fn default() -> Self {
        Self {
            stature_cm: 165,
            mass_kg: 65,
            fitness: 50,
            strength: 50,
            conditioning_experience: 0,
        }
    }
}

/// Mutable personality dimensions. These are tendencies, not roles or action limits.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Personality {
    /// Interest in novelty and unfamiliar experiences (0–100).
    pub openness: u8,
    /// Preference for order and follow-through (0–100).
    pub conscientiousness: u8,
    /// Social energy (0–100).
    pub extraversion: u8,
    /// Tendency toward trust and cooperation (0–100).
    pub agreeableness: u8,
    /// Emotional steadiness under stress (0–100).
    pub emotional_stability: u8,
}

impl Default for Personality {
    fn default() -> Self {
        Self {
            openness: 50,
            conscientiousness: 50,
            extraversion: 50,
            agreeableness: 50,
            emotional_stability: 50,
        }
    }
}

/// Kind of a resident's subjective attitude.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttitudeKind {
    /// A claim the resident treats as probably true.
    Belief,
    /// A personal judgment that may differ from another resident's.
    Opinion,
    /// A preferred or disliked experience or outcome.
    Preference,
}

/// Private belief, opinion, preference, or impression about another resident.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PersonalAttitude {
    /// Attitude category.
    pub kind: AttitudeKind,
    /// Subject, such as `sharing`, `risk`, `water`, or `person`.
    pub topic: String,
    /// Optional person this attitude concerns. Relationship-like impressions
    /// emerge from interactions and are not initialized as fixed bonds.
    pub target: Option<ResidentId>,
    /// Leaning from -100 (strongly against) through 0 (uncertain) to 100.
    pub leaning: i8,
    /// Strength of conviction from 0 to 100.
    pub conviction: u8,
    /// Most recent experience that updated this attitude.
    pub updated_at: VirtualTime,
}

/// Where a personal recollection came from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemorySource {
    /// The resident made or directly experienced the choice.
    SelfExperience,
    /// The resident witnessed another resident's action.
    Observed,
}

/// A private, subjective recollection; it is not authoritative world history.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PersonalMemory {
    /// First-person recollection, including the resident's interpretation.
    pub recollection: String,
    /// Resident whose action or presence is part of the remembered event.
    pub about: Option<ResidentId>,
    /// Event category used to connect the recollection to current interests.
    #[serde(default)]
    pub topic: String,
    /// How the memory formed.
    pub source: MemorySource,
    /// When the event happened in virtual time.
    pub formed_at: VirtualTime,
    /// Current vividness, from 0 (forgotten) to 100 (vivid).
    pub strength: u8,
    /// Personal significance when the memory formed (0–100).
    pub significance: u8,
    /// Relevance to the resident's interests and goals when formed (0–100).
    pub relevance: u8,
    /// Confidence in the recollection (0–100), not objective accuracy.
    pub confidence: u8,
    /// Emotional tone from -100 (very negative) to 100 (very positive).
    pub emotional_tone: i8,
}
