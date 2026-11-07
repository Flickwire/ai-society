# AI Society — Native Rust Simulation

An offline-first autonomous society in a local native application, with a Bevy 3D viewer, renderer-free headless runner, accelerated virtual time, persistent memory and independently configurable inference providers.

**Status:** initial runnable headless version. It creates and resumes a persistent
eight-resident village with deterministic needs and a SQLite checkpoint store.

## Architectural decisions

- **Entire application in Rust.** No TypeScript, JavaScript runtime, Node.js services, browser frontend, or web deployment.
- **Bevy native viewer** for exploration, inspection, time controls and playback.
- **Pure Rust simulation core**, independent of Bevy rendering, wall-clock ticks and GPU resources.
- **Separate headless executable** for maximum-speed experiments, benchmarks and unattended runs.
- **SQLite/WAL** with append-only event history, snapshots, migrations and experiment branching.
- **Swappable inference** through a Rust provider trait: LM Studio by default through its OpenAI-compatible API, or any configured compatible endpoint.
- **Single-machine only** initially; no external-facing server, public observer or multiplayer requirement.
- **Hardware target:** RX 7700 XT (12 GB VRAM), 64 GB RAM. OS/CPU to confirm.

## Workspace

```text
crates/
  society-domain/       # shared IDs, actions, resources and events
  society-sim/          # deterministic world rules and state transitions
    src/types.rs        # residents, objects, social records, wilderness seed
    src/personal.rs     # physical/personality/attitude/memory data
    src/inner_life.rs   # memory decay and experiential changes
    src/autonomy.rs     # need- and trait-sensitive no-model policy
    src/village.rs      # world state, snapshots, initialization and queries
    src/validation.rs   # action preconditions against current state
    src/actions.rs      # committed action effects and virtual-time updates
    src/rules.rs        # spatial, inventory, harvesting and crafting rules
    src/clock.rs        # simulation clock and scheduled events
  society-inference/    # OpenAI-compatible provider and protocol types
  society-storage/     # SQLite WAL snapshots
  society-headless/    # CLI entry point and agent decision loop
    src/agent.rs        # prompt context, action parsing and safe fallback
  migrations/           # SQLite schema
assets/
migrations/
experiments/
docs/
```

Keep files focused on one responsibility. Add an action by updating its domain
schema, simulator validation and effects, then the agent's supported-action
prompt. Keep inference transport in `society-inference`, world rules in
`society-sim`, and CLI wiring in `society-headless`. The root
[`AGENTS.md`](AGENTS.md) gives development commands and boundaries.

The current executable creates a wilderness with eight residents, twelve
landmarks, and twenty natural-object clusters. The 3D block-coordinate world has
trees, rock, water, berries, wild grain, fiber, clay, caves, deer, rabbits, and
fish. Residents have hunger, thirst, health, energy, morale, inventories,
individual physical and cognitive profiles, personality dimensions, private
attitudes, and memories. They can move, gather, hunt, fish, drink, eat, build, destroy,
dig, plant, harvest, craft, share resources, talk, play, invent, and request or
vote on missing simulator capabilities. SQLite stores validated snapshots in
WAL mode, and each `run` resumes from the latest snapshot.

Actors remember their own choices; nearby witnesses form separate impressions
and may misread intent. Memory vividness decays each virtual hour, with more
significant and relevant memories fading more slowly. Memories and attitudes
are shown only to that resident's inference prompt; use `inspect` to study an
individual from the researcher CLI. Residents start without assigned
occupations or self-chosen goals. Relationship-like opinions form through
interaction rather than a preassigned social graph.

Without `--inference`, `run` uses a small deterministic policy driven by
immediate needs and resident traits. It has no market schedule, assigned job,
or fixed relationship pattern; `inspect` helps compare individual changes.

```sh
cargo run -p society-headless -- init --world village.sqlite
cargo run -p society-headless -- run --world village.sqlite --hours 168
cargo run -p society-headless -- doctor --world village.sqlite
cargo run -p society-headless -- inspect --world village.sqlite --resident 1
cargo run -p society-headless -- inference models
cargo run -p society-headless -- inference prompt --text "Say hello in one sentence"
cargo run -p society-headless -- run --world village.sqlite --hours 24 --inference
cargo run -p society-headless -- feedback --world village.sqlite
```

Inference defaults to LM Studio at `http://localhost:1234/v1`. Use
`--base-url https://provider.example/v1 --model MODEL_ID --api-key-env KEY_ENV`
to select another OpenAI-compatible service; the key is read from the named
environment variable. Non-loopback endpoints must use HTTPS, and credentials
must not be embedded in the URL. `AI_SOCIETY_BASE_URL`, `AI_SOCIETY_MODEL`, and
`AI_SOCIETY_API_KEY` can configure defaults; `AI_SOCIETY_API_KEY_ENV` can change
the key variable name. With `--inference`, each resident
gets one model decision per virtual hour. Outputs are parsed as typed actions
and validated by the simulation; connection failures, malformed outputs, and
invalid actions use a safe deterministic fallback. Residents cannot add
executable behavior themselves: an unavailable action is recorded as a
capability request, and a majority vote marks it accepted or declined for
future implementation. The world is currently a bounded discrete grid (x/z
from -64 to 64, y from 0 to 16); movement can traverse up to twelve blocks per
action, and structures appear immediately when their material costs are met.

Build with `cargo build --workspace`; use `--help` for CLI options. Long-term
semantic memory and reflection, durable action-event replay, richer
construction projects, and the Bevy viewer remain planned milestones.

## Plans

- [Implementation](docs/IMPLEMENTATION.md)
- [Deployment and operations](docs/DEPLOYMENT.md)
- [Cost and performance benchmarking](docs/COSTS-AND-BENCHMARKS.md)

## Core contract

LLMs propose typed actions; the simulation validates and commits them. Renderer state is a projection of the simulation, not authoritative state. Virtual time advances through causally ordered events, with inference backpressure. Replay reuses recorded model outputs for deterministic reconstruction.

## Not in v1

Public hosting, JavaScript stack, networked multiplayer, real-money economy, blockchain, unrestricted agent shell/network access, foundation-model training, and claims of guaranteed emergence or consciousness.
