//! Execute bounded, read-only state queries and format concise results.

use super::schema::StateQuestion;
use society_sim::{
    CapabilityStatus, ConversationLine, PersonalAttitude, PersonalMemory, Resident, Village,
    WorldObject,
};

pub(super) fn answer_questions(
    village: &Village,
    resident: &Resident,
    questions: Vec<StateQuestion>,
) -> Vec<String> {
    questions
        .into_iter()
        .map(|question| match question {
            StateQuestion::Inventory => format!("Inventory: {}", format_inventory(resident)),
            StateQuestion::NearbyObjects { radius, limit } => format!(
                "Nearby objects within {radius}: {}",
                format_objects(village, resident, radius, limit)
            ),
            StateQuestion::Memories {
                query,
                about,
                limit,
            } => format!(
                "Personal memories matching {:?} about {:?}: {}",
                query,
                about,
                format_memories(resident, query.as_deref(), about, limit)
            ),
            StateQuestion::Notes { limit } => format!(
                "Your durable notes: {}",
                resident
                    .notes
                    .iter()
                    .take(usize::from(limit))
                    .map(|note| format!(
                        "#{} (written {}, updated {}): {}",
                        note.id, note.created_at.0, note.updated_at.0, note.text
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            StateQuestion::Attitudes {
                topic,
                about,
                limit,
            } => format!(
                "Personal attitudes matching topic {:?} about {:?}: {}",
                topic,
                about,
                format_attitudes(village, resident, topic.as_deref(), about, limit)
            ),
            StateQuestion::NearbyPeople { radius, limit } => format!(
                "Nearby people: {}",
                format_people(village, resident, radius, limit)
            ),
            StateQuestion::Conversations { with, limit } => format!(
                "Recent conversations with {:?}: {}",
                with,
                format_conversations(village, resident, with, limit)
            ),
            StateQuestion::OpenCapabilityRequests { limit } => format!(
                "Open capability requests: {}",
                format_requests(village, limit)
            ),
            StateQuestion::Inventions { query, limit } => format!(
                "Recorded inventions matching {:?}: {}",
                query,
                format_inventions(village, query.as_deref(), limit)
            ),
            StateQuestion::Places { query, limit } => format!(
                "Landmarks matching {:?}: {}",
                query,
                format_places(village, resident, query.as_deref(), limit)
            ),
        })
        .collect()
}

pub(super) fn append_query_results(context: &mut String, results: Vec<String>) {
    const MAX_CONTEXT_BYTES: usize = 6_000;
    let mut omitted = false;
    for result in results {
        let separator = usize::from(!context.is_empty());
        if context.len() + separator + result.len() > MAX_CONTEXT_BYTES {
            omitted = true;
            continue;
        }
        if separator > 0 {
            context.push('\n');
        }
        context.push_str(&result);
    }
    if omitted {
        let note = "Some requested results were omitted to keep context bounded.";
        let separator = usize::from(!context.is_empty());
        if context.len() + separator + note.len() <= MAX_CONTEXT_BYTES {
            if separator > 0 {
                context.push('\n');
            }
            context.push_str(note);
        }
    }
}

fn format_inventory(resident: &Resident) -> String {
    if resident.inventory.is_empty() && resident.food == 0 {
        return "empty".to_owned();
    }
    let mut items: Vec<_> = resident
        .inventory
        .iter()
        .map(|(resource, amount)| format!("{resource:?} {amount}"))
        .collect();
    if resident.food > 0 {
        items.push(format!("food {}", resident.food));
    }
    items.join(", ")
}

pub(super) fn format_objects(
    village: &Village,
    resident: &Resident,
    radius: u8,
    limit: u8,
) -> String {
    let mut objects: Vec<_> = village
        .world_objects()
        .iter()
        .filter(|object| resident.position.distance(object.position) <= u32::from(radius))
        .collect();
    objects.sort_by_key(|object| resident.position.distance(object.position));
    if objects.is_empty() {
        return "none in range".to_owned();
    }
    objects
        .into_iter()
        .take(usize::from(limit))
        .map(|object| format_object(object, resident))
        .collect::<Vec<_>>()
        .join("; ")
}

fn format_object(object: &WorldObject, resident: &Resident) -> String {
    format!(
        "#{} {} ({:?}) at ({},{},{}), distance {}, stock {}",
        object.id.0,
        object.name,
        object.kind,
        object.position.x,
        object.position.y,
        object.position.z,
        resident.position.distance(object.position),
        object.quantity
    )
}

fn format_memories(
    resident: &Resident,
    query: Option<&str>,
    about: Option<u64>,
    limit: u8,
) -> String {
    let terms: Vec<_> = query
        .unwrap_or_default()
        .to_ascii_lowercase()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let mut memories: Vec<_> = resident
        .memories
        .iter()
        .filter(|memory| {
            memory.strength > 0
                && about.is_none_or(|id| {
                    memory.about.is_some_and(|person| person.0 == id)
                        || (id == resident.id.0 && memory.about.is_none())
                })
                && (terms.is_empty()
                    || terms.iter().any(|term| {
                        memory.topic.to_ascii_lowercase().contains(term)
                            || memory.recollection.to_ascii_lowercase().contains(term)
                    }))
        })
        .collect();
    memories.sort_by_key(|memory| {
        std::cmp::Reverse(
            u32::from(memory.strength)
                * u32::from(memory.significance)
                * u32::from(memory.relevance),
        )
    });
    if memories.is_empty() {
        return "no matching recollections".to_owned();
    }
    memories
        .into_iter()
        .take(usize::from(limit))
        .map(|memory| format_memory(memory))
        .collect::<Vec<_>>()
        .join("; ")
}

fn format_memory(memory: &PersonalMemory) -> String {
    format!(
        "hour {} [{}; strength {}, significance {}, relevance {}, confidence {}]: {}",
        memory.formed_at.0,
        memory.topic,
        memory.strength,
        memory.significance,
        memory.relevance,
        memory.confidence,
        memory.recollection
    )
}

fn format_attitudes(
    village: &Village,
    resident: &Resident,
    topic: Option<&str>,
    about: Option<u64>,
    limit: u8,
) -> String {
    let mut attitudes: Vec<&PersonalAttitude> = resident
        .attitudes
        .iter()
        .filter(|attitude| {
            topic.is_none_or(|topic| {
                attitude
                    .topic
                    .to_ascii_lowercase()
                    .contains(&topic.to_ascii_lowercase())
            }) && about.is_none_or(|id| attitude.target.is_some_and(|person| person.0 == id))
        })
        .collect();
    attitudes.sort_by_key(|attitude| std::cmp::Reverse(attitude.conviction));
    if attitudes.is_empty() {
        return "no matching attitudes".to_owned();
    }
    attitudes
        .into_iter()
        .take(usize::from(limit))
        .map(|attitude| {
            let target = attitude.target.map_or_else(String::new, |id| {
                let name = village
                    .residents()
                    .iter()
                    .find(|other| other.id == id)
                    .map_or_else(|| format!("#{}", id.0), |other| other.name.clone());
                format!(" about {name}")
            });
            format!(
                "{:?} {}{} (leaning {}, conviction {})",
                attitude.kind, attitude.topic, target, attitude.leaning, attitude.conviction
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn format_people(village: &Village, resident: &Resident, radius: u8, limit: u8) -> String {
    let mut people: Vec<_> = village
        .residents()
        .iter()
        .filter(|other| {
            other.id != resident.id
                && resident.position.distance(other.position) <= u32::from(radius)
        })
        .collect();
    people.sort_by_key(|other| resident.position.distance(other.position));
    if people.is_empty() {
        return "no one nearby".to_owned();
    }
    people
        .into_iter()
        .take(usize::from(limit))
        .map(|other| {
            format!(
                "{} id {} at ({},{},{}), distance {}, morale {}",
                other.name,
                other.id.0,
                other.position.x,
                other.position.y,
                other.position.z,
                resident.position.distance(other.position),
                other.morale
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn format_conversations(
    village: &Village,
    resident: &Resident,
    with: Option<u64>,
    limit: u8,
) -> String {
    let mut lines: Vec<&ConversationLine> = village
        .conversations()
        .iter()
        .filter(|line| {
            (line.from == resident.id || line.to == resident.id)
                && with.is_none_or(|id| {
                    (line.from == resident.id && line.to.0 == id)
                        || (line.to == resident.id && line.from.0 == id)
                })
        })
        .collect();
    lines.sort_by_key(|line| std::cmp::Reverse(line.at));
    if lines.is_empty() {
        return "no matching conversations".to_owned();
    }
    lines
        .into_iter()
        .take(usize::from(limit))
        .map(|line| format!("hour {}: {}", line.at.0, line.text))
        .collect::<Vec<_>>()
        .join("; ")
}

fn format_requests(village: &Village, limit: u8) -> String {
    let requests: Vec<_> = village
        .capability_requests()
        .iter()
        .filter(|request| request.status == CapabilityStatus::Open)
        .take(usize::from(limit))
        .map(|request| {
            let yes = request.votes.iter().filter(|vote| vote.support).count();
            let no = request.votes.len() - yes;
            format!(
                "#{} {}: {} ({} support, {} oppose)",
                request.id.0, request.name, request.description, yes, no
            )
        })
        .collect::<Vec<_>>();
    if requests.is_empty() {
        "none".to_owned()
    } else {
        requests.join("; ")
    }
}

fn format_inventions(village: &Village, query: Option<&str>, limit: u8) -> String {
    let query = query.unwrap_or_default().to_ascii_lowercase();
    let inventions: Vec<_> = village
        .inventions()
        .iter()
        .filter(|invention| {
            query.is_empty()
                || invention.name.to_ascii_lowercase().contains(&query)
                || invention.description.to_ascii_lowercase().contains(&query)
        })
        .take(usize::from(limit))
        .map(|invention| {
            format!(
                "{} by resident {}: {}",
                invention.name, invention.inventor.0, invention.description
            )
        })
        .collect::<Vec<_>>();
    if inventions.is_empty() {
        "none matching".to_owned()
    } else {
        inventions.join("; ")
    }
}

fn format_places(village: &Village, resident: &Resident, query: Option<&str>, limit: u8) -> String {
    let query = query.unwrap_or_default().to_ascii_lowercase();
    let mut places: Vec<_> = village
        .places()
        .iter()
        .filter(|place| query.is_empty() || place.name.to_ascii_lowercase().contains(&query))
        .collect();
    places.sort_by_key(|place| resident.position.distance(place.position));
    if places.is_empty() {
        return "none matching".to_owned();
    }
    places
        .into_iter()
        .take(usize::from(limit))
        .map(|place| {
            format!(
                "{} at ({},{},{}) distance {}",
                place.name,
                place.position.x,
                place.position.y,
                place.position.z,
                resident.position.distance(place.position)
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub(super) fn supported_action_schema() -> &'static str {
    "move_to(destination:{x,y,z}), speak(to,text), eat, eat_resource(food,amount), rest(duration), gather(target,amount), drink(source), hunt(target), fish(target,amount), build(structure,position), destroy(target), dig(position), plant(seed,position), harvest(target,amount), craft_item(recipe), share_resource(to,resource,amount), play(activity,participants,duration), set_goal(goal), keep_note(text), update_note(note,text), delete_note(note), invent(name,description,materials), propose_capability(name,description,example), vote_capability(proposal,support). Notes are durable private notes you choose to keep; ask for notes before editing or deleting by ID. Structures: lean_to, fire_pit, storage_rack, workbench, garden, fence, raft. Recipes: stone_axe, spear, rope, basket, net. Resources: wood, stone, fiber, clay, grain, berries, grain_seed, berry_seed, meat, fish."
}

pub(super) fn supported_query_schema() -> &'static str {
    "{\"kind\":\"inventory\"}; {\"kind\":\"nearby_objects\",\"radius\":8,\"limit\":6}; {\"kind\":\"memories\",\"query\":\"...\",\"limit\":4} (optional numeric about); {\"kind\":\"notes\",\"limit\":8}; {\"kind\":\"attitudes\",\"topic\":\"...\",\"limit\":6} (optional numeric about); {\"kind\":\"nearby_people\",\"radius\":8,\"limit\":6}; {\"kind\":\"conversations\",\"limit\":4} (optional numeric with); {\"kind\":\"open_capability_requests\",\"limit\":4}; {\"kind\":\"inventions\",\"query\":\"...\",\"limit\":4}; {\"kind\":\"places\",\"query\":\"...\",\"limit\":4}"
}
