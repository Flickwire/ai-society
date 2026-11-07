//! Command-line interface for running a persistent village without graphics.

mod agent;

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};
use society_inference::{
    ChatMessage, ChatRequest, DEFAULT_BASE_URL, InferenceProvider, OpenAiCompatibleProvider,
    ProviderConfig,
};
use society_sim::{SimulationClock, Village};
use society_storage::Store;

#[derive(Debug, Parser)]
#[command(version, about = "Renderer-free AI Society simulation runner")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a new world in a SQLite database.
    Init {
        /// Path to the world database.
        #[arg(long, default_value = "ai-society.sqlite")]
        world: PathBuf,
    },
    /// Advance a world by virtual hours and save a checkpoint.
    Run {
        /// Path to the world database.
        #[arg(long, default_value = "ai-society.sqlite")]
        world: PathBuf,
        /// Number of virtual hours to simulate (maximum one year).
        #[arg(long, default_value_t = 24, value_parser = clap::value_parser!(u64).range(0..=8760))]
        hours: u64,
        /// Print only the run summary and final resident state.
        #[arg(short, long)]
        quiet: bool,
        /// Ask the configured LLM for a typed action from every resident each hour.
        #[arg(long)]
        inference: bool,
        /// Inference provider settings (default: local LM Studio).
        #[command(flatten)]
        provider: ProviderOptions,
    },
    /// Check the headless simulation and storage layer.
    Doctor {
        /// Path to the world database to inspect.
        #[arg(long, default_value = "ai-society.sqlite")]
        world: PathBuf,
    },
    /// List agent proposals for capabilities missing from the simulator.
    Feedback {
        /// Path to the world database.
        #[arg(long, default_value = "ai-society.sqlite")]
        world: PathBuf,
    },
    /// Connect to a configured OpenAI-compatible inference server.
    Inference {
        #[command(subcommand)]
        command: InferenceCommand,
    },
}

#[derive(Debug, Args, Clone)]
struct ProviderOptions {
    /// OpenAI-compatible API base URL. Defaults to LM Studio on localhost:1234.
    #[arg(long, env = "AI_SOCIETY_BASE_URL", default_value = DEFAULT_BASE_URL)]
    base_url: String,
    /// Provider model ID. If omitted, the first model returned by /models is used.
    #[arg(long, env = "AI_SOCIETY_MODEL")]
    model: Option<String>,
    /// Environment variable containing the optional bearer token.
    #[arg(
        long,
        env = "AI_SOCIETY_API_KEY_ENV",
        default_value = "AI_SOCIETY_API_KEY"
    )]
    api_key_env: String,
    /// Per-request timeout in seconds.
    #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u64).range(1..=3600))]
    timeout_seconds: u64,
}

#[derive(Debug, Subcommand)]
enum InferenceCommand {
    /// List models exposed by the configured server.
    Models {
        #[command(flatten)]
        provider: ProviderOptions,
    },
    /// Send a prompt to the configured model and print its response.
    Prompt {
        /// Prompt text.
        #[arg(long)]
        text: String,
        #[command(flatten)]
        provider: ProviderOptions,
    },
}

impl ProviderOptions {
    fn create(&self) -> Result<OpenAiCompatibleProvider, Box<dyn std::error::Error>> {
        let api_key = std::env::var(&self.api_key_env).ok();
        Ok(OpenAiCompatibleProvider::new(ProviderConfig {
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            api_key,
            timeout: Duration::from_secs(self.timeout_seconds),
        })?)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::Init { world } => {
            let store = Store::open(&world).await?;
            if store.load().await?.is_some() {
                return Err(format!("world already exists at {}", world.display()).into());
            }
            let village = Village::new();
            store.save(&village).await?;
            println!(
                "Created wilderness at {} (8 residents, 12 landmarks, 20 natural objects).",
                world.display()
            );
        }
        Command::Run {
            world,
            hours,
            quiet,
            inference,
            provider: provider_options,
        } => {
            let store = Store::open(&world).await?;
            let mut village = match store.load().await? {
                Some(village) => village,
                None => {
                    let village = Village::new();
                    store.save(&village).await?;
                    village
                }
            };
            println!(
                "Starting at virtual hour {} with {} residents, {} wilderness landmarks, and {} world objects.",
                village.now().0,
                village.residents().len(),
                village.places().len(),
                village.world_objects().len()
            );
            if !quiet {
                for place in village.places() {
                    println!(
                        "  landmark {}: {} at ({},{},{})",
                        place.id.0,
                        place.name,
                        place.position.x,
                        place.position.y,
                        place.position.z
                    );
                }
            }
            let reports = if inference {
                let provider = provider_options.create()?;
                agent::run_with_inference(&mut village, &store, hours, quiet, &provider).await?
            } else {
                village
                    .run_for(hours)
                    .map_err(|error| format!("simulation stopped: {error:?}"))?
            };
            let action_count: usize = reports.iter().map(|report| report.actions.len()).sum();
            if !quiet {
                for report in &reports {
                    if report.time.0 % 6 == 0 || report.time.0 == village.now().0 {
                        println!("\nHour {}", report.time.0);
                        for action in &report.actions {
                            println!("  {action}");
                        }
                    }
                }
            }
            store.save(&village).await?;
            println!(
                "\nSaved {} actions at virtual hour {} (world version {}) to {}.",
                action_count,
                village.now().0,
                village.version().0,
                world.display()
            );
            println!("\nResidents:");
            for resident in village.residents() {
                println!(
                    "  {} — hunger {}, thirst {}, energy {}, health {}, morale {}, position ({},{},{}), inventory {:?}, goal: {}",
                    resident.name,
                    resident.hunger,
                    resident.thirst,
                    resident.energy,
                    resident.health,
                    resident.morale,
                    resident.position.x,
                    resident.position.y,
                    resident.position.z,
                    resident.inventory,
                    resident.goal
                );
            }
        }
        Command::Doctor { world } => {
            let store = Store::open(&world).await?;
            let clock = SimulationClock::default();
            println!("simulation core: ready");
            println!("SQLite WAL storage: ready");
            println!(
                "empty simulation clock: tick {}, version {}",
                clock.now().0,
                clock.version().0
            );
            match store.load().await? {
                Some(village) => {
                    println!(
                        "world: ready ({} residents, virtual hour {}, version {})",
                        village.residents().len(),
                        village.now().0,
                        village.version().0
                    );
                    let structures = village
                        .world_objects()
                        .iter()
                        .filter(|object| {
                            matches!(object.kind, society_sim::WorldObjectKind::Structure(_))
                        })
                        .count();
                    println!(
                        "wilderness: {} objects, {structures} structures, {} inventions, {} capability requests",
                        village.world_objects().len(),
                        village.inventions().len(),
                        village.capability_requests().len()
                    );
                }
                None => println!("world: no saved world at {}", world.display()),
            }
            println!("inference: use `society-headless inference models` to check a provider");
            println!("graphical viewer: not implemented yet");
        }
        Command::Feedback { world } => {
            let store = Store::open(&world).await?;
            match store.load().await? {
                Some(village) if !village.capability_requests().is_empty() => {
                    for request in village.capability_requests() {
                        let support = request.votes.iter().filter(|vote| vote.support).count();
                        let oppose = request.votes.len() - support;
                        println!(
                            "#{} {} — {:?} ({support} support, {oppose} oppose)",
                            request.id.0, request.name, request.status
                        );
                        println!("  {}", request.description);
                        println!("  Example: {}", request.example);
                        println!("  Proposed by resident {}", request.requested_by.0);
                    }
                }
                Some(_) => println!("No agent capability requests yet."),
                None => println!("No saved world at {}.", world.display()),
            }
        }
        Command::Inference { command } => match command {
            InferenceCommand::Models { provider } => {
                let provider = provider.create()?;
                for model in provider.list_models().await? {
                    println!("{}", model.id);
                }
            }
            InferenceCommand::Prompt { text, provider } => {
                let provider = provider.create()?;
                let response = provider
                    .chat(ChatRequest {
                        messages: vec![ChatMessage::new("user", text)],
                        max_tokens: 512,
                        temperature: 0.7,
                        response_format: None,
                        reasoning_effort: Some("low".to_owned()),
                    })
                    .await?;
                println!("{}", response.content);
                if let Some(model) = response.model {
                    eprintln!("model: {model}");
                }
            }
        },
    }
    Ok(())
}
