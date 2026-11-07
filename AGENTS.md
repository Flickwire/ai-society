# Agent development guide

This file describes the repository as it exists today. Read it before extending
the simulator; use `docs/IMPLEMENTATION.md` for the longer-term roadmap.

## Current state

- Rust 2024 Cargo workspace; toolchain is pinned in `rust-toolchain.toml`.
- Existing crates: `society-domain`, `society-sim`, `society-storage`,
  `society-inference`, and `society-headless`.
- The runnable product is the headless CLI. There is no Bevy viewer, graphical
  app, cognition crate, event replay system, or long-term agent memory yet.
- A new world has eight residents, twelve wilderness landmarks, and twenty
  natural-object clusters. Coordinates are bounded (x/z: -64..=64, y: 0..=16);
  one movement action can cover up to twelve blocks. The world is discrete and
  has no collision or pathfinding system.
- Treat the implementation as authoritative when it disagrees with aspirational
  wording in roadmap or product documentation.

## Repository map

- `crates/society-domain/src/lib.rs`: stable IDs, `Position`, resource and
  structure types, `Action`, proposals, events, and rejection reasons.
- `crates/society-sim/src/types.rs`: resident/object/social records and initial
  wilderness seed data.
- `crates/society-sim/src/village.rs`: authoritative world state, snapshot
  serialization/validation, initialization, and read-only queries.
- `crates/society-sim/src/validation.rs`: action preconditions and bounds.
- `crates/society-sim/src/actions.rs`: committed state changes and clock ticks.
- `crates/society-sim/src/rules.rs`: reusable coordinate, inventory, harvest,
  construction, and craft rules; `clock.rs` owns virtual time and versions.
- `crates/society-inference/src/lib.rs`: provider trait and OpenAI-compatible
  HTTP implementation. Default base URL: `http://localhost:1234/v1`.
- `crates/society-headless/src/main.rs`: CLI commands and wiring.
  `crates/society-headless/src/agent.rs`: perception prompt, typed response
  parsing, supported-action check, and safe fallback policy.
- `crates/society-storage/src/lib.rs`: SQLite connection, schema migration,
  complete snapshot save/load. SQL schema is in `migrations/`.

Keep these dependency directions: domain types do not depend on other crates;
the simulator depends on the domain only; storage and inference are adapters;
the CLI composes them. Do not add HTTP, SQL, Tokio, CLI, or rendering concerns
to `society-domain` or `society-sim`.

## Simulation contract

Agents submit typed proposals. The simulator is authoritative and checks the
resident, expected world version, idempotency key, action preconditions,
coordinates, proximity, quantities, and resource costs before applying an
action. When adding or changing an action, update all relevant layers:

1. Add/update the typed payload in `society-domain::Action`.
2. Implement its preconditions in `Village::action_is_valid` in `validation.rs`.
3. Implement deterministic effects in `Village::apply` in `actions.rs`.
4. Add it to the accepted-action match and prompt/action guidance in
   `headless/src/agent.rs`.
5. Update user-facing help/docs if the capability is visible to players.

Keep validation and effects consistent. Reject invalid proposals without
partial state changes. Preserve stale-version protection and increment the
version once for a successful transition. Keep world rules deterministic so
headless runs remain reproducible. Capability proposals and votes are stored
feedback only: never turn their text into executable code or tools.

The model response is untrusted input. An agent may choose only typed simulator
actions; agents must not gain shell, filesystem, process, or arbitrary network
access as a side effect of an action change. Validate lengths, counts, IDs,
positions, inventory capacity, and recipient/object proximity at the simulator
boundary, even when the prompt tells the model those same rules.

## Persistence and compatibility

- SQLite is opened in WAL mode with one pooled connection. The current store
  appends complete JSON world snapshots and restores the latest one.
- The schema migration creates metadata, event-log, and snapshot tables, but
  action events are not currently written to the event log and replay is not
  implemented. Do not describe snapshots as event sourcing.
- World snapshot format is version 2. `Village::from_snapshot` accepts formats
  1 and 2 and upgrades v1 data in memory. If changing serialized state, decide
  whether to add a format migration and preserve existing database behavior.
- Committed idempotency keys are in-memory and are not persisted across restart.
- Use a new temporary database path for manual scenarios, e.g.
  `/tmp/ai-society-dev.sqlite`; do not overwrite a user's saved world.

## Inference notes

- Default is LM Studio at `localhost:1234`; model defaults to the first ID from
  `/models`. Select a model with `--model` or `AI_SOCIETY_MODEL`.
- Remote OpenAI-compatible endpoints use `--base-url` and optional bearer
  token environment variable. Non-loopback URLs require HTTPS; never put a key
  directly in a URL or commit credentials.
- The client requests one action per resident per virtual hour. Invalid output
  or a provider error uses deterministic fallback behavior; after the first
  provider error, inference is disabled for the remainder of that run.
- Providers differ in structured-output and reasoning support. Keep optional
  compatibility fields optional and parse only the response content, never
  hidden reasoning fields. A local model can be reachable yet fail to return a
  parseable action; report that distinction accurately.

## Development workflow

From the workspace root:

```sh
cargo fmt --all
cargo check --workspace
cargo run -p society-headless -- --help
cargo run -p society-headless -- init --world /tmp/ai-society-dev.sqlite
cargo run -p society-headless -- run --world /tmp/ai-society-dev.sqlite --hours 1
cargo run -p society-headless -- doctor --world /tmp/ai-society-dev.sqlite
```

Use the smallest relevant CLI scenario after behavior changes. Inference smoke
runs require a running compatible endpoint; test provider connectivity with
`cargo run -p society-headless -- inference models` first. No test suite is
currently present; add focused tests alongside new core behavior when practical.

Keep source files responsibility-focused. Split a module before it becomes a
mixed collection of state, rules, transport, and command wiring. Prefer small
changes with clear ownership, and use Conventional Commit subjects such as
`feat(sim): add shelter repair`, `fix(storage): reject invalid snapshots`, or
`refactor(agent): split prompt assembly`.
