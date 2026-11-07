# Development guide

## Build and run

- Format Rust code with `cargo fmt --all`.
- Check the full workspace with `cargo check --workspace`.
- Start a world with `cargo run -p society-headless -- init --world /tmp/village.sqlite`.
- Advance it with `cargo run -p society-headless -- run --world /tmp/village.sqlite --hours 1`.
- Check CLI options with `cargo run -p society-headless -- --help`.

## Code boundaries

- `society-domain` owns serializable IDs, action payloads, and simulation events.
- `society-sim` owns authoritative rules. Keep it independent of HTTP, storage,
  CLI, and rendering. Add world types in `types.rs`, state and snapshots in
  `village.rs`, preconditions in `validation.rs`, action effects in `actions.rs`,
  reusable mechanics in `rules.rs`, and time bookkeeping in `clock.rs`.
- `society-inference` owns provider configuration and OpenAI-compatible HTTP.
- `society-storage` owns persistence and migrations.
- `society-headless` owns command routing. Resident prompt construction,
  response validation, and safe fallbacks belong in `agent.rs`.
- Keep one responsibility per file. Split a module before it grows into a broad
  mix of world state, rules, and IO.

## Adding an agent capability

1. Add a typed action in `society-domain`.
2. Define its preconditions in `society-sim/src/validation.rs`.
3. Apply its deterministic state changes in `society-sim/src/actions.rs`.
4. Add it to the resident action guidance and response validator in
   `society-headless/src/agent.rs`.
5. Update the README when the capability changes what a resident can do.

Model output is untrusted input. The simulator remains authoritative; agent
proposals cannot execute arbitrary code or modify files, processes, or network
state. Capability votes are feedback for developers and never activate code.

## Change hygiene

Keep commits focused and use Conventional Commit subjects such as
`feat(sim): add shelter repair` or `refactor(agent): split prompt assembly`.
After a change, run formatting and the workspace check. For behavior changes,
also exercise the smallest relevant CLI scenario and add focused regression
coverage where practical.
