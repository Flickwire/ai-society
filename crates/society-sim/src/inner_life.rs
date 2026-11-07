//! Personal memory and slow individual change from lived events.

use super::personal::{
    AttitudeKind, MemorySource, PersonalAttitude, PersonalMemory, Personality, PhysicalProfile,
};
use super::types::Resident;
use society_domain::{Action, ResidentId, VirtualTime};

const MAX_MEMORIES_PER_RESIDENT: usize = 128;
const MAX_ATTITUDES_PER_RESIDENT: usize = 48;

pub(super) fn initial_intelligence(id: u64) -> u8 {
    35 + ((id * 29 + 7) % 61) as u8
}

pub(super) fn initial_physical(id: u64) -> PhysicalProfile {
    PhysicalProfile {
        stature_cm: 148 + ((id * 17 + 3) % 43) as u16,
        mass_kg: 47 + ((id * 13 + 9) % 37) as u16,
        fitness: 32 + ((id * 23 + 5) % 66) as u8,
        strength: 28 + ((id * 31 + 11) % 68) as u8,
        conditioning_experience: 0,
    }
}

pub(super) fn initial_personality(id: u64) -> Personality {
    let score = |multiplier: u64, offset: u64| 20 + ((id * multiplier + offset) % 81) as u8;
    Personality {
        openness: score(17, 3),
        conscientiousness: score(29, 19),
        extraversion: score(37, 7),
        agreeableness: score(43, 31),
        emotional_stability: score(53, 13),
    }
}

pub(super) fn initial_attitudes(id: u64) -> Vec<PersonalAttitude> {
    [
        (AttitudeKind::Preference, "exploration", 19, 7),
        (AttitudeKind::Preference, "sharing", 31, 17),
        (AttitudeKind::Preference, "solitude", 47, 29),
        (AttitudeKind::Belief, "nature_is_predictable", 59, 41),
        (AttitudeKind::Opinion, "change_is_worthwhile", 71, 53),
    ]
    .into_iter()
    .map(|(kind, topic, multiplier, offset)| PersonalAttitude {
        kind,
        topic: topic.to_owned(),
        target: None,
        leaning: (((id * multiplier + offset) % 161) as i16 - 80) as i8,
        conviction: 20 + ((id * (multiplier + 11) + offset) % 66) as u8,
        updated_at: VirtualTime(0),
    })
    .collect()
}

/// Record separate first-person accounts for the actor and nearby witnesses.
/// Witness accounts include an inferred motive, which may be wrong and is marked
/// with lower confidence. These memories are never copied between residents.
pub(super) fn record_action_memories(
    residents: &mut [Resident],
    actor_id: ResidentId,
    action: &Action,
    observers: &[ResidentId],
    at: VirtualTime,
) {
    let Some(actor) = residents.iter().find(|resident| resident.id == actor_id) else {
        return;
    };
    let actor_name = actor.name.clone();
    let actor_position = actor.position;
    let action_text = describe_action(action);
    let topic = action_topic(action);
    let important_need = actor.hunger.max(actor.thirst);
    let base_significance: u8 = match action {
        Action::Invent { .. } | Action::ProposeCapability { .. } => 75,
        Action::Speak { .. } | Action::ShareResource { .. } | Action::Play { .. } => 58,
        Action::Hunt { .. } | Action::Build { .. } | Action::Destroy { .. } => 62,
        Action::MoveTo { .. } | Action::Move { .. } | Action::Observe => 32,
        _ => 42,
    };

    for resident in residents {
        let is_actor = resident.id == actor_id;
        if !is_actor && !observers.contains(&resident.id) {
            continue;
        }
        let significance = base_significance
            .saturating_add(u8::from(important_need >= 70) * 15)
            .min(100);
        let goal_relevance = u8::from(
            resident
                .goal
                .to_ascii_lowercase()
                .split_whitespace()
                .any(|word| action_text.to_ascii_lowercase().contains(word)),
        ) * 24;
        let attitude_relevance = u8::from(
            resident
                .attitudes
                .iter()
                .any(|attitude| attitude.topic == topic),
        ) * 18;
        let relevance = (35 + goal_relevance + attitude_relevance).min(100);
        let source = if is_actor {
            MemorySource::SelfExperience
        } else {
            MemorySource::Observed
        };
        let confidence = if is_actor {
            (35 + resident.intelligence / 2).min(95)
        } else {
            let distance = resident.position.distance(actor_position);
            (25 + resident.intelligence / 2)
                .saturating_sub(8 + u8::try_from(distance.saturating_mul(3)).unwrap_or(u8::MAX))
                .max(15)
        };
        let recollection = if is_actor {
            format!("I remember choosing to {action_text}. I felt this mattered to me.")
        } else {
            let interpretation = inferred_motive(resident, topic);
            format!(
                "I remember seeing {actor_name} {action_text}. I guessed they did it {interpretation}; I may have misunderstood."
            )
        };
        resident.memories.push(PersonalMemory {
            recollection,
            about: (!is_actor).then_some(actor_id),
            topic: topic.to_owned(),
            source,
            formed_at: at,
            strength: (55 + significance / 3).min(100),
            significance,
            relevance,
            confidence,
            emotional_tone: if resident.morale >= 60 { 12 } else { -12 },
        });
        if resident.memories.len() > MAX_MEMORIES_PER_RESIDENT {
            resident
                .memories
                .sort_by_key(|memory| (memory.strength, memory.formed_at));
            resident.memories.remove(0);
        }
    }
}

/// Gradually fade memories. Significant, relevant memories persist longer;
/// higher intelligence slightly improves retention without making memory exact.
pub(super) fn decay_memories(residents: &mut [Resident], now: VirtualTime) {
    for resident in residents {
        for memory in &mut resident.memories {
            let related_goal = !resident.goal.is_empty()
                && resident
                    .goal
                    .to_ascii_lowercase()
                    .split_whitespace()
                    .filter(|word| word.len() > 3)
                    .any(|word| memory.recollection.to_ascii_lowercase().contains(word));
            let attitude_relevance = resident
                .attitudes
                .iter()
                .filter(|attitude| {
                    attitude.topic == memory.topic
                        || memory
                            .about
                            .is_some_and(|person| attitude.target == Some(person))
                })
                .map(|attitude| attitude.conviction)
                .max()
                .unwrap_or(0);
            let current_relevance =
                (25 + u8::from(related_goal) * 35 + attitude_relevance / 2).min(100);
            memory.relevance =
                ((u16::from(memory.relevance) * 3 + u16::from(current_relevance)) / 4) as u8;
            let decay = 1
                + u16::from(100 - memory.significance) / 20
                + u16::from(100 - memory.relevance) / 20;
            let retention = u16::from(resident.intelligence / 50);
            let loss = decay.saturating_sub(retention).max(1).min(100) as u8;
            memory.strength = memory.strength.saturating_sub(loss);
        }
        resident.memories.retain(|memory| {
            memory.strength > 0 && now.0.saturating_sub(memory.formed_at.0) < 24 * 365 * 10
        });
    }
}

/// Evolve physical conditioning, intelligence, personality, and personal views
/// in small increments as a consequence of selected lived experiences.
pub(super) fn evolve_from_action(
    residents: &mut [Resident],
    actor_id: ResidentId,
    action: &Action,
    at: VirtualTime,
) {
    let Some(actor_index) = residents
        .iter()
        .position(|resident| resident.id == actor_id)
    else {
        return;
    };
    {
        let actor = &mut residents[actor_index];
        if matches!(
            action,
            Action::MoveTo { .. }
                | Action::Gather { .. }
                | Action::Hunt { .. }
                | Action::Fish { .. }
                | Action::Build { .. }
                | Action::Destroy { .. }
                | Action::Dig { .. }
        ) {
            actor.physical.conditioning_experience += 1;
            if actor.physical.conditioning_experience >= 8 {
                actor.physical.conditioning_experience = 0;
                actor.physical.fitness = actor.physical.fitness.saturating_add(1).min(100);
                if !matches!(action, Action::MoveTo { .. }) {
                    actor.physical.strength = actor.physical.strength.saturating_add(1).min(100);
                }
            }
        }
        if matches!(
            action,
            Action::Observe
                | Action::Craft { .. }
                | Action::CraftItem { .. }
                | Action::Invent { .. }
                | Action::Dig { .. }
                | Action::Plant { .. }
        ) {
            actor.learning_experience += 1;
            if actor.learning_experience >= 6 {
                actor.learning_experience = 0;
                actor.intelligence = actor.intelligence.saturating_add(1).min(100);
            }
        }
        match action {
            Action::Play { .. } | Action::Speak { .. } => {
                actor.personality.extraversion =
                    actor.personality.extraversion.saturating_add(1).min(100);
            }
            Action::ShareResource { .. } => {
                actor.personality.agreeableness =
                    actor.personality.agreeableness.saturating_add(1).min(100);
            }
            Action::Invent { .. } => {
                actor.personality.openness = actor.personality.openness.saturating_add(1).min(100);
            }
            Action::SetGoal { .. } => {
                actor.personality.conscientiousness = actor
                    .personality
                    .conscientiousness
                    .saturating_add(1)
                    .min(100);
            }
            _ => {}
        }
    }

    match action {
        Action::Speak { to, .. } => {
            update_relationships(residents, actor_id, *to, 1, at);
            if let Some(recipient) = residents.iter().find(|resident| resident.id == *to) {
                let response =
                    i8::try_from(recipient.personality.agreeableness / 25).unwrap_or(4) - 2;
                update_relationships(residents, *to, actor_id, response, at);
            }
        }
        Action::ShareResource { to, .. } => {
            update_relationships(residents, actor_id, *to, 4, at);
            update_relationships(residents, *to, actor_id, 7, at);
            shift_topic(&mut residents[actor_index], "sharing", 2, at);
        }
        Action::Play { participants, .. } => {
            for participant in participants {
                update_relationships(residents, actor_id, *participant, 4, at);
                update_relationships(residents, *participant, actor_id, 4, at);
            }
        }
        Action::Invent { .. } | Action::Craft { .. } | Action::CraftItem { .. } => {
            shift_topic(&mut residents[actor_index], "making", 3, at);
        }
        Action::MoveTo { .. } => shift_topic(&mut residents[actor_index], "exploration", 2, at),
        Action::Hunt { .. } | Action::Gather { .. } | Action::Fish { .. } => {
            shift_topic(&mut residents[actor_index], "foraging", 2, at);
        }
        _ => {}
    }
}

/// Daily deprivation changes body mass and conditioning; recovery can restore
/// them gradually. Height remains stable until the simulation models age.
pub(super) fn update_body_from_needs(residents: &mut [Resident], at: VirtualTime) {
    for resident in residents {
        if resident.hunger >= 90 || resident.thirst >= 90 {
            resident.physical.mass_kg = resident.physical.mass_kg.saturating_sub(1).max(30);
            resident.personality.emotional_stability =
                resident.personality.emotional_stability.saturating_sub(1);
            resident.personality.agreeableness =
                resident.personality.agreeableness.saturating_sub(1);
            shift_topic(resident, "nature_is_predictable", -1, at);
            shift_topic(resident, "security", -2, at);
            if resident.health < 60 {
                resident.physical.fitness = resident.physical.fitness.saturating_sub(1);
                resident.physical.strength = resident.physical.strength.saturating_sub(1);
            }
        } else if resident.hunger < 35 && resident.thirst < 35 && resident.health >= 80 {
            resident.physical.mass_kg = resident.physical.mass_kg.saturating_add(1).min(120);
            resident.physical.fitness = resident.physical.fitness.saturating_add(1).min(100);
            resident.personality.emotional_stability = resident
                .personality
                .emotional_stability
                .saturating_add(1)
                .min(100);
            shift_topic(resident, "security", 1, at);
        }
    }
}

fn update_relationships(
    residents: &mut [Resident],
    from: ResidentId,
    target: ResidentId,
    change: i8,
    at: VirtualTime,
) {
    if from == target {
        return;
    }
    if let Some(resident) = residents.iter_mut().find(|resident| resident.id == from) {
        let attitude = resident
            .attitudes
            .iter_mut()
            .find(|attitude| attitude.topic == "person" && attitude.target == Some(target));
        if let Some(attitude) = attitude {
            attitude.leaning = attitude.leaning.saturating_add(change).clamp(-100, 100);
            attitude.conviction = attitude.conviction.saturating_add(2).min(100);
            attitude.updated_at = at;
        } else if resident.attitudes.len() < MAX_ATTITUDES_PER_RESIDENT {
            resident.attitudes.push(PersonalAttitude {
                kind: AttitudeKind::Opinion,
                topic: "person".to_owned(),
                target: Some(target),
                leaning: change,
                conviction: 12,
                updated_at: at,
            });
        }
    }
}

fn shift_topic(resident: &mut Resident, topic: &str, change: i8, at: VirtualTime) {
    if let Some(attitude) = resident
        .attitudes
        .iter_mut()
        .find(|attitude| attitude.topic == topic && attitude.target.is_none())
    {
        attitude.leaning = attitude.leaning.saturating_add(change).clamp(-100, 100);
        attitude.conviction = attitude.conviction.saturating_add(1).min(100);
        attitude.updated_at = at;
    } else if resident.attitudes.len() < MAX_ATTITUDES_PER_RESIDENT {
        resident.attitudes.push(PersonalAttitude {
            kind: AttitudeKind::Preference,
            topic: topic.to_owned(),
            target: None,
            leaning: change,
            conviction: 10,
            updated_at: at,
        });
    }
}

fn inferred_motive(resident: &Resident, topic: &str) -> &'static str {
    if !matches!(topic, "sharing" | "making" | "foraging" | "exploration") {
        return "for reasons I do not know";
    }
    let leaning = resident
        .attitudes
        .iter()
        .find(|attitude| attitude.topic == topic)
        .map_or(0, |attitude| attitude.leaning);
    if leaning > 35 || resident.personality.agreeableness > 75 {
        "to help or cooperate"
    } else if leaning < -35 || resident.personality.emotional_stability < 30 {
        "for their own benefit"
    } else {
        "for reasons I do not know"
    }
}

fn action_topic(action: &Action) -> &'static str {
    match action {
        Action::Move { .. } | Action::MoveTo { .. } => "exploration",
        Action::Gather { .. } | Action::Hunt { .. } | Action::Fish { .. } => "foraging",
        Action::Speak { .. } | Action::Play { .. } | Action::ShareResource { .. } => "sharing",
        Action::Craft { .. } | Action::CraftItem { .. } | Action::Invent { .. } => "making",
        Action::Eat | Action::EatResource { .. } | Action::Drink { .. } => "survival",
        _ => "change",
    }
}

fn describe_action(action: &Action) -> String {
    match action {
        Action::MoveTo { destination } => format!(
            "walk toward ({},{},{})",
            destination.x, destination.y, destination.z
        ),
        Action::Move { destination } => format!("travel to place {}", destination.0),
        Action::Observe => "look over the surroundings".to_owned(),
        Action::Speak { to, text } => format!(
            "speak with resident {} about saying “{}”",
            to.0,
            truncate_to_bytes(text, 300)
        ),
        Action::Eat | Action::EatResource { .. } => "eat food".to_owned(),
        Action::Drink { .. } => "drink from water".to_owned(),
        Action::Rest { duration } => format!("rest for {duration} hours"),
        Action::Gather { target, amount } => format!(
            "gather {amount} {} near object {}",
            if *amount == 1 { "unit" } else { "units" },
            target.0
        ),
        Action::Hunt { target } => format!("hunt animal {}", target.0),
        Action::Fish { target, amount } => format!("catch {amount} fish near object {}", target.0),
        Action::Build { structure, .. } => format!("build a {structure:?}"),
        Action::Destroy { target } => format!("destroy object {}", target.0),
        Action::Dig { .. } => "dig into the ground".to_owned(),
        Action::Plant { seed, .. } => format!("plant {seed:?}"),
        Action::Harvest { target, amount } => {
            format!("harvest {amount} units from object {}", target.0)
        }
        Action::Craft { recipe } => format!("craft {recipe}"),
        Action::CraftItem { recipe } => format!("craft {recipe:?}"),
        Action::ShareResource { to, amount, .. } => {
            format!("share {amount} resources with resident {}", to.0)
        }
        Action::Play { activity, .. } => format!("take part in {activity}"),
        Action::SetGoal { goal } => format!("choose the goal {goal}"),
        Action::KeepNote { .. } => "keep a personal note".to_owned(),
        Action::UpdateNote { .. } => "update a personal note".to_owned(),
        Action::DeleteNote { .. } => "delete a personal note".to_owned(),
        Action::Invent { name, .. } => format!("experiment with the idea {name}"),
        Action::ProposeCapability { name, .. } => format!("request the new activity {name}"),
        Action::VoteCapability { proposal, support } => format!(
            "{} capability request {}",
            if *support { "support" } else { "oppose" },
            proposal.0
        ),
        Action::Forage { .. } => "forage for food".to_owned(),
        Action::Work { job, .. } => format!("work at {job}"),
        Action::StartProject { project } => format!("start project {project}"),
        Action::ContinueProject { project, .. } => format!("continue project {project}"),
        Action::CancelProject { project } => format!("cancel project {project}"),
        Action::OfferTrade { to, .. } => format!("offer a trade to resident {}", to.0),
        Action::AcceptTrade { .. } => "accept a trade".to_owned(),
    }
}

fn truncate_to_bytes(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut boundary = max_bytes;
    while !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &text[..boundary]
}
