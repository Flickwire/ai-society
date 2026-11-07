//! Periodic, read-only chronicle generation for CLI observers.

use std::collections::{BTreeMap, HashMap};

use society_inference::{ChatMessage, ChatRequest, InferenceProvider};
use society_sim::{TickReport, VillageSnapshot, WorldObject, WorldObjectKind};

const MAX_ACTION_KINDS: usize = 32;
const MAX_WORLD_CHANGES: usize = 24;

pub(super) async fn summarize_period(
    provider: &impl InferenceProvider,
    reports: &[TickReport],
    before: &VillageSnapshot,
    after: &VillageSnapshot,
) -> Result<String, Box<dyn std::error::Error>> {
    let Some(first) = reports.first() else {
        return Err("storyteller received an empty period".into());
    };
    let last = reports
        .last()
        .expect("a first report guarantees a last report");
    let action_evidence = summarize_actions(reports);
    let resident_evidence = summarize_residents(after);
    let world_changes = summarize_world_changes(before, after);
    let prompt = format!(
        "Virtual hours {}–{}. Actions recorded: {}.\nAction patterns (counts):\n{}\nResident status at the end of the period:\n{}\nSignificant world changes during the period:\n{}",
        first.time.0,
        last.time.0,
        reports
            .iter()
            .map(|report| report.actions.len())
            .sum::<usize>(),
        action_evidence,
        resident_evidence,
        world_changes
    );
    let response = provider
        .chat(ChatRequest {
            messages: vec![
                ChatMessage::new(
                    "system",
                    "You are an outside chronicler of a simulated primitive society. Write a concise, engaging CLI chronicle in 2–5 sentences, at most 130 words. Summarize what residents did during the stated hours, where they are and what they currently aim for, and any notable changes to the shared environment. Use only the supplied evidence. Do not invent dialogue, motives, relationships, ongoing tasks, or events. Distinguish recorded actions from current status. If there were no major environmental changes, say so plainly. This narration is observational and must not suggest or cause future actions.",
                ),
                ChatMessage::new("user", prompt),
            ],
            max_tokens: 256,
            temperature: 0.5,
            response_format: None,
            reasoning_effort: Some("low".to_owned()),
        })
        .await?;
    let summary = response
        .content
        .trim()
        .trim_start_matches("```text")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    if summary.is_empty() {
        return Err(format!(
            "storyteller returned empty text (finish reason: {:?})",
            response.finish_reason
        )
        .into());
    }
    Ok(summary.chars().take(1_200).collect())
}

fn summarize_actions(reports: &[TickReport]) -> String {
    let mut counts = BTreeMap::<String, usize>::new();
    for action in reports.iter().flat_map(|report| &report.actions) {
        *counts.entry(action.clone()).or_default() += 1;
    }
    let mut counts: Vec<_> = counts.into_iter().collect();
    counts.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let summary = counts
        .into_iter()
        .take(MAX_ACTION_KINDS)
        .map(|(action, count)| format!("{action} ({count})"))
        .collect::<Vec<_>>();
    if summary.is_empty() {
        "No resident actions were committed.".to_owned()
    } else {
        summary.join("; ")
    }
}

fn summarize_residents(snapshot: &VillageSnapshot) -> String {
    snapshot
        .residents
        .iter()
        .map(|resident| {
            let place = snapshot
                .places
                .iter()
                .find(|place| place.id == resident.place)
                .map_or("unmapped", |place| place.name.as_str());
            let goal = if resident.goal.is_empty() {
                "has no self-chosen goal"
            } else {
                resident.goal.as_str()
            };
            format!(
                "{}: at {} ({},{},{}), hunger {}, thirst {}, morale {}; goal: {}",
                resident.name,
                place,
                resident.position.x,
                resident.position.y,
                resident.position.z,
                resident.hunger,
                resident.thirst,
                resident.morale,
                goal
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn summarize_world_changes(before: &VillageSnapshot, after: &VillageSnapshot) -> String {
    let old_objects: HashMap<_, _> = before
        .world_objects
        .iter()
        .map(|object| (object.id, object))
        .collect();
    let new_objects: HashMap<_, _> = after
        .world_objects
        .iter()
        .map(|object| (object.id, object))
        .collect();
    let mut changes = Vec::new();

    for object in &after.world_objects {
        if !old_objects.contains_key(&object.id) && is_notable_object(object) {
            changes.push(format!(
                "created {} ({:?}) at ({},{},{})",
                object.name, object.kind, object.position.x, object.position.y, object.position.z
            ));
        }
    }
    for object in &before.world_objects {
        if !new_objects.contains_key(&object.id) && is_notable_object(object) {
            changes.push(format!("removed {} ({:?})", object.name, object.kind));
        }
        if let Some(current) = new_objects.get(&object.id) {
            let difference = object.quantity.abs_diff(current.quantity);
            if difference >= 4 || (object.quantity > 0 && current.quantity == 0) {
                changes.push(format!(
                    "{} stock changed from {} to {}",
                    object.name, object.quantity, current.quantity
                ));
            }
        }
    }
    for invention in &after.inventions {
        if !before
            .inventions
            .iter()
            .any(|old| old.name == invention.name && old.inventor == invention.inventor)
        {
            changes.push(format!("recorded invention: {}", invention.name));
        }
    }
    for request in &after.capability_requests {
        if !before
            .capability_requests
            .iter()
            .any(|old| old.id == request.id)
        {
            changes.push(format!("new capability request: {}", request.name));
        }
    }
    if changes.is_empty() {
        "No major structural, resource-stock, or invention changes were recorded.".to_owned()
    } else {
        changes.truncate(MAX_WORLD_CHANGES);
        changes.join("; ")
    }
}

fn is_notable_object(object: &WorldObject) -> bool {
    !matches!(object.kind, WorldObjectKind::WaterSource)
}
