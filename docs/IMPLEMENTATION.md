# Implementation plan — fully native Rust

## Current implementation status

The first runnable headless slice is implemented: workspace/toolchain, typed
domain actions and events, deterministic village needs, a renderer-free CLI,
SQLite WAL schema, checkpoint save/restore, and an OpenAI-compatible inference
provider defaulting to LM Studio on `localhost:1234`. `run --inference` asks
each resident for a typed action every virtual hour and validates it before
commit. The starter environment has eight residents, twelve landmarks, and
twenty wilderness object clusters with 3D positions. Implemented actions cover
survival, movement, harvesting, hunting, fishing, construction, destruction,
crafting, social exchange, recreation, goals, and invention records. Residents
can propose unavailable actions and vote; a majority result is stored for
future implementation and never becomes executable code automatically. Long-
term memory, durable action event replay, richer construction projects, and the
Bevy application remain future milestones. The full eight-week feature set is
not yet implemented.

## 1. Definition of done

Eight autonomous residents inhabit a persistent village with 12–20 locations, homes, market, workshop, shared resources and wilderness. They move, rest, eat, forage, craft, trade, converse, remember, form relationships and pursue evolving goals. A native Bevy application displays and inspects the town; a separate headless executable runs identical rules without loading graphics. An unattended seven-virtual-day run must survive restarts, preserve transaction invariants, record reasoning traces and support exact replay from stored model outputs.

## 2. Project structure and libraries

Rust Cargo workspace, pinned stable toolchain and Cargo.lock. Bevy 0.20 (verify compatibility at implementation), Tokio, Reqwest, Serde, schema generation/validation, SQLx SQLite, clap, tracing, metrics, proptest, criterion and cargo-deny/audit. Keep simulation-domain crates free of Bevy, HTTP, Tokio and GPU dependencies. Bevy ECS is used for visual entities; authoritative simulation uses deterministic serialisable domain structures. Share stable IDs and typed projection events between them.

| Crate | Responsibilities |
|---|---|
| society-domain | IDs, events, action schemas, world entities, rule versioning |
| society-sim | Event priority queue, virtual time, deterministic state transitions |
| society-cognition | Perception assembly, memory, goals, wake-up policy, reflection |
| society-inference | Provider trait, adapters, routing, usage/cost limits |
| society-storage | SQLite migrations, transactions, snapshots, replay |
| society-view | Bevy 3D scene, interpolation, inspector and timeline |
| society-app | Native app composition, interactive controls |
| society-headless | No graphics initialization; fast-forward CLI |
| society-bench | GPU throughput and world load tests |

## 3. World model and authoritative rules

Use a graph of places, links and objects, with residents holding location/transit, needs, inventory, money (integer minor units), skills, current activity, relationships, goals and identity. Define actions such as Move, Observe, Speak, Eat, Rest, Forage, Craft, Work, OfferTrade, AcceptTrade, StartProject, ContinueProject and CancelProject. Actions carry typed parameters, expected world version, stable idempotency ID and preconditions. Only the single simulation writer applies changes, in a transaction with event append and version increment.

All simulation consequences are deterministic given versioned rules, input action and seeded PRNG. Needs advance analytically over elapsed virtual time; travel and work schedule completion events. Precompute paths or path costs. Do not perform per-frame physics updates in headless mode. Failed purchases and other conflicts return explicit feedback events instead of corrupting balances.

## 4. Virtual-time scheduler

Priority queue sorted by (virtual_time_tick, priority, sequence_id). Jump directly to the next meaningful event; quiet hours need not simulate every frame. Maintain independent monotonic wall duration, UTC operation timestamp and integer virtual time. Agent reasoning is asynchronous: a versioned perception is placed in a bounded worker queue; the simulation waits when causal ordering requires a decision. Outdated proposals are rejected and reconsidered. Parallel inference never commits simultaneous mutations directly.

Modes:

- **realtime** — virtual clock follows wall time.
- **capped** — attempt a configurable multiplier; slow automatically when the model is behind.
- **fast-forward** — run as fast as pending events and model inference allow.
- **replay** — no inference calls; use recorded responses, random outcomes and periodic snapshots.

Store event/state hashes to identify replay divergence. Same model prompts do not imply deterministic fresh generations; exact reproduction requires stored outputs.

## 5. Inference isolation and routing

Rust trait:

```rust
#[async_trait::async_trait]
pub trait InferenceProvider: Send + Sync {
    async fn generate(&self, req: InferenceRequest)
        -> Result<InferenceResponse, InferenceError>;
    fn capabilities(&self) -> ProviderCapabilities;
}
```

Transport implementations: OpenAI-compatible JSON HTTP for llama.cpp and supported commercial endpoints; add a dedicated adapter only when unavoidable. Requests carry role (routine/dialogue/planning/reflection/embedding), messages, supported structured-output schema, token caps, timeout, trace ID. Responses carry provider/model, content, token usage, cache usage if reported, latency and billed/estimated cost. Capability negotiation checks JSON schema/tool support, context length, streaming, reasoning tokens and caching; never assume universal API equivalence.

Configure per-role ordered providers/models. Default **local only**, llama.cpp on loopback, with no remote keys. Remote use is explicit opt-in, with a separate allowlist, daily/monthly budgets and fail-closed behavior. Route routine actions to quantised 4B–9B models; optionally use larger hosted models for planning/reflection. A provider switch must require configuration only, not world-rule changes. Retain an inference compatibility test suite against every configured endpoint.

## 6. Agent autonomy

Wake triggers: completed task, nearby speech, arriving visitor/agent, need threshold, goal deadline, changed resources, scheduled curiosity and nightly reflection. Every agent has a minimum think interval, next review time, priority, bounded token allowance and activity continuation. A walk or repetitive work activity should not spend tokens every frame. A normal decision returns 1–3 typed actions and reasoning, ideally from a 1–2K token perception and <=300 output tokens. Strictly validate actions; on malformed output or timeout, optionally retry once then continue safe existing activity or rest.

Memory: short working context, episodic experiences, semantic facts, subjective relationships, promises, goals, baseline personality and bounded amendments. Persist provenance, owner, virtual timestamp, salience, confidence, recall frequency and decay. Start retrieval with SQLite FTS5 and structured filters; evaluate local embedding/vector search later using measured retrieval quality. Agents only see their own memories and permitted world observations. During rest, a separate bounded reflection updates summaries, beliefs and goals without deleting protected original identity.

## 7. Native Bevy GUI

Stylised low-poly town, agent animation and interpolated paths, day/night, selected-resident inspector, event history, reason/action validity, memory and relationships view, profiler, inference settings, pause/speed/fast-forward, snapshots and replay timeline. Keep render FPS separately configurable (e.g. 15/30/60) because it competes with local inference for RX 7700 XT GPU capacity. The visual ECS is a read model only; closing the window does not corrupt simulation state. Headless CLI never loads Bevy render plugins or requests a graphics device.

## 8. Persistence and experiments

SQLite WAL, one writer, read-only inspector connection, migrations and durable transactions. Tables for world metadata, entity state, inventories, goals, relationships, memories, decision traces, event log, snapshots and experiment branches. Store integer virtual ticks and monotonic sequence. Snapshots daily or by event count; compressed and verified, with append-only events after each snapshot. Branch by snapshot + rules/config/seed/model identity. Exact replay reads captured decisions instead of rerunning the LLM. Avoid storing secrets or unnecessary visitor personal content in traces.

## 9. Performance engineering

Baseline before tuning: 8 agents × 8 decisions/virtual hour × 2K input/300 output tokens. Keep context around 8K and start with 1 inference slot on the RX 7700 XT; benchmark 2 and 4 slots and Q4/Q5 before increasing. One model server is shared among all residents. Avoid full world clones per perception, use compact IDs, spatial indexes and cached path costs, batch bounded database writes, and separate blocking IO from the world reducer. Bounded queues and backpressure are mandatory. Profile actual end-to-end throughput, not just LLM decode tokens/sec.

## 10. Development milestones

1. **Foundation (2–4 evenings):** Cargo workspace, CI, strict lint, domain actions/events, schema migrations, minimal headless binary. Gate: clean build/test on target OS; no JS toolchain.
2. **Simulation (weeks 1–2):** seeded town, eight residents, needs, navigation, resources, work/rest/eat actions, event store, snapshots. Gate: 30 deterministic virtual days with conservation/property tests.
3. **Inference (week 3):** llama.cpp local endpoint, Rust adapter, schema-validation, event wake-ups, budget/fallback behavior. Gate: 24 simulated hours; malformed outputs cannot alter world; model benchmarks recorded.
4. **Society (weeks 4–5):** persistent memory, FTS, conversation, goals, relationships, trades, reflection. Gate: after restart an agent recalls a person, completes a multi-step task and atomic trade; no private-memory leaks.
5. **Bevy app (week 6):** town renderer, controls, inspector, replay. Gate: GUI and headless share identical state transitions; GUI restart safe.
6. **Optimization (week 7):** adaptive acceleration, profiling, WAL recovery, queue stress, memory and VRAM limits. Gate: seven virtual days unattended with zero integrity failures and measured acceleration.
7. **Providers/experiments (week 8):** opt-in remote adapter, model-role configuration, hard caps, experiment branches and comparison metrics. Gate: switch local/remote via config only; budget exhaustion fails closed; replay checksums match.

## 11. Tests and security

Unit/proptest: time integration, inventory/currency conservation, event monotonicity, bounded needs and queues, deterministic PRNG. Integration: DB crash/restart, concurrent conflicting purchases, late model responses, malformed JSON, provider outage, snapshot restoration. Replay: checkpoint hashes identical when recorded inference is used. Security: prompt injection through dialogue/memory never escalates to shell, filesystem, network or secret access. No unrestricted agent tools in v1. CI runs fmt, clippy, test, audit and release build; native packaging after endurance gates.

## 12. Unresolved preferences

CPU and OS, art style, licensing, resident identities, and whether optional remote model providers may process private interactions. These do not block the core architecture.
