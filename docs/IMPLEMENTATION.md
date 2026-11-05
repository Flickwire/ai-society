# Detailed implementation plan

## 1. Scope and acceptance criteria

Build an eight-resident persistent society in a small town containing housing, a market, workshop, public square, garden, forest, shared well, and 12–20 interactive locations. Residents can move, rest, eat, forage, craft, trade, communicate, form relationships, remember experiences, establish goals, and reflect on prior behaviour. Visitors initially observe only. 3D display is a read-only projection of authoritative simulation state.

Success at v1: eight residents survive an unattended seven-day simulated run; at least one multi-step goal and one validated trade can complete; restart preserves state; replay reconstructs the same world when recorded decisions are reused; local inference hard-limits concurrency; no model output can bypass action validation. Measure rather than promise emergent social behaviour.

## 2. Monorepo and stack

Use TypeScript strict mode, pnpm workspaces, Turborepo, Node.js LTS, Zod, Vitest, Playwright, React, React Three Fiber, Colyseus, PostgreSQL/pgvector, Drizzle ORM, Redis/BullMQ (introduce after prototype), OpenTelemetry and structured JSON logs. llama.cpp exposes a local OpenAI-compatible API. Containerise all services except native GPU inference initially.

```text
apps/
  web/                 # Three.js viewer, playback controls, observer UI
  world-server/        # Colyseus rooms, commands, authoritative state
  agent-worker/        # inference, reflection, perception, action plans
  admin/               # traces, budgets, snapshots, experiment controls
packages/
  domain/              # branded IDs, event/action types, Zod schemas
  simulation/          # deterministic rules and virtual-time clock
  cognition/           # prompt construction, decision policy, routing
  memory/              # episodic/social/semantic retrieval and decay
  inference/           # local/cloud adapters and token accounting
  experiments/         # snapshots, branches, replay, metrics
infra/                 # Compose, migrations, config examples
docs/
```

## 3. Authoritative world model

Represent the world as a graph of locations, traversable edges and interactive objects. Each actor has location, optional transit, inventory, money, need levels, skills, current task, relationships, a version, and a stable identity. All movements and object interactions become append-only domain events. Use a transaction to atomically validate preconditions, apply effects, increment versions and append events.

Suggested tables: `worlds`, `world_clock`, `agents`, `agent_state`, `locations`, `edges`, `objects`, `inventories`, `inventory_items`, `resources`, `relationships`, `goals`, `tasks`, `promises`, `memories`, `beliefs`, `world_events`, `decision_requests`, `decision_results`, `simulation_snapshots`, `experiment_branches`, `model_usage`.

Use integer minor units for currency and integer item quantities. Stable IDs and optimistic state versions prevent duplicated purchases. Seedable PRNG governs deterministic world effects; store seeds and rule versions. UTC timestamps describe real operations, while an integer `virtual_time_ms` orders simulated events. Never use wall-clock `Date.now()` in game-rule evaluation.

## 4. Simulation engine and acceleration

Implement a priority queue ordered by `(virtual_time_ms, priority, sequence_id)`. The engine advances directly to the next event. Deterministic periodic needs accrue analytically over elapsed virtual time, rather than performing one update per rendered frame. Timed actions have start, finish, interruption and cancellation events. Physics/paths can be precomputed as travel durations for headless simulation; the renderer interpolates from published positions.

Modes:
- **Real-time:** virtual clock follows monotonic wall time.
- **Capped:** target multiplier (e.g. 3x or 5x), applying backpressure when inference lags.
- **Fast-forward:** process next event as soon as required model outputs become available; no promised fixed multiplier.
- **Replay:** use recorded model decisions and random outcomes, never issue fresh inference; allow seek from snapshots.

Determinism policy: all actions due at a virtual timestamp resolve in stable order; observations are built against a versioned state. LLM decisions can execute only if preconditions remain valid. Failed actions become feedback events. Do not let wall-clock latency alter simulation chronology. A queue item carries its virtual causality timestamp and snapshot version.

## 5. Cognition policy

The agent scheduler reacts to: completed action, nearby speech/arrival, environmental hazard, unmet need threshold, goal deadline, periodic curiosity and sleep/wake transitions. Each agent has a minimum think interval, interrupt priority, maximum prompt and completion tokens, per-virtual-day call budget, and real-dollar budget. For ordinary passive travel or work, continue a multi-minute action without new inference.

Build prompts from: immutable identity card; concise rules; current local observation; needs; available tool actions; current commitments; top-k relevant memories; and recent causal events. Keep ordinary prompts around 1,000–2,000 tokens and responses under 300 tokens initially. Prefer structured output like `{actions:[{kind,args,reason}], next_check_virtual_ms}` validated with Zod. Reject unknown actions, invalid targets, impossible inventory transfers, overlong outputs and world-state contradictions. On timeout, continue ongoing action or use deterministic rest behaviour.

Initial model tests: Qwen3.5-9B Q4_K_M vs Q5_K_M, and a small 4B contender. Run both against identical scenario fixtures. Reserve a hosted stronger model for optional complicated planning/reflection; never depend on it for core simulation operation.

## 6. Memory and identity

Distinguish working context, episodic memories, stable semantic facts, subjective social beliefs, commitments and immutable baseline identity. Memories carry source event, observer agent, virtual timestamp, importance, confidence, retrieval count, optional embedding, decay parameters and supersession links. Retrieval scores combine semantic similarity, recency, salience, relationship and unfinished-task relevance. Maintain strict per-agent visibility: agent A cannot inspect B's private records.

Consolidate during scheduled rest: summarize repeated experiences, reinforce useful facts, evaluate unfinished goals, record tentative preference changes and prune low-value episodes. Keep a locked baseline personality; any evolving traits are versioned and reversible. Preserve raw event records separately, so lossy memory pruning does not erase the audit trail.

## 7. Social and economic simulation

Give each agent an individual worldview and relationship ledger, not one universal friendship score. Dialogue is audible within a radius or shared room. Messages are events; an agent may listen, respond, ignore or leave. Cap back-and-forth exchanges to prevent infinite token-consuming conversations. Trade follows `offer -> accept -> atomic transfer`, with expiry and cancellation. Scarcity should be tunable per experiment, without hard-coding social outcomes. No real money in v1.

## 8. Observability and experiment design

Log decision ID, virtual time, prompt template revision, provider/model, input/output/cached tokens, wall latency, cost, selected actions, validation outcomes and snapshot versions. Store redacted prompt/response traces locally with configurable retention. Admin UI shows individual timelines, relationships, goals, inventory, memory retrievals, token budgets, heatmaps and virtual-time lag.

Experiments require immutable starting snapshot, simulation rules hash, agent prompt version, model versions, PRNG seed and model outputs. A fresh run with the same prompt is not guaranteed deterministic; exact replay reuses recorded outputs. Compare model cohorts on action validity, goals completed, resource inequality, reciprocal interactions, repetition, errors and cost per simulated day. Make behavioral claims cautiously.

## 9. Safety and access control

Treat agent conversation, visitor messages and memories as untrusted prompt content. Tool access is a closed allowlist; structured actions may only address in-world objects and identities. No shell, network browsing, external messaging, purchasing or unrestricted file writes in v1. If later adding creative code tools, execute in a disposable sandbox with resource/time/network limits and explicit approval gates for publication. Never expose model API keys or internal prompts to browser clients. Apply auth to the admin panel; rate-limit public interactions if opened to visitors.

## 10. Phased delivery backlog

### Phase 0 — Foundation (2–4 evenings)
- Initialise repo, license decision, pnpm workspace, TypeScript strict, lint/format, CI, Docker Compose, migrations, seed fixtures.
- Define architecture decision records (ADRs), event/action schemas and measurable budgets.
- Exit: reproducible `pnpm test`, database migration and headless process from clean checkout.

### Phase 1 — Deterministic town (week 1–2)
- Map graph, 8 seeded residents, needs, time, inventories, locations, timed move/rest/eat/forage actions.
- Implement transactional event store, snapshot restore and reproducible no-LLM fast-forward.
- Exit: 30 virtual days in headless deterministic simulation; no negative inventories or duplicate items.

### Phase 2 — Inference and scheduler (week 3)
- llama.cpp adapter, structured output validation, event wake-ups, per-agent concurrency and deadlines.
- Build fixed scenario corpus and cost/latency traces; fallback on model failure.
- Exit: 24-hour simulated run with no unhandled tool or schema exceptions; benchmark 1/2/4 concurrent requests.

### Phase 3 — Memory, goals and relationships (week 4–5)
- Episodic memory, semantic retrieval, subjective relationship records, commitments, reflection.
- Add dialogue hearing, exchange protocols and multi-step task planning.
- Exit: agents remember relevant events after restart; complete a trade and multi-step goal; privacy tests pass.

### Phase 4 — Viewer and operator experience (week 6)
- Browser 3D map, character animations, selection inspector, live activity feed, clock controls.
- Admin controls for pause/resume, speed cap, snapshot, experiment branch and budget changes.
- Exit: client reconnects cleanly; headless simulation does not require viewer.

### Phase 5 — Performance and reliability (week 7)
- Profile GPU inference, prompt prefill, queue lag, memory retrieval and database writes.
- Implement adaptive backpressure, compact snapshots, retention, service restart and backups.
- Exit: seven consecutive virtual days with 8 agents, zero data corruption, bounded queues and no budget overruns.

### Phase 6 — Experiments and packaging (week 8)
- A/B scenarios with common initial snapshot; metrics exports and comparison dashboard.
- Prepare private local deployment docs and optional hosted fallback switch.
- Exit: same recorded run replays identically; fresh branch can vary scarcity/model settings.

## 11. Test strategy

- Unit: needs integration across elapsed time, deterministic PRNG, action schema, goals, decay and ranking.
- Property-based: conservation of items/currency, unique ownership, no negative inventory, bounded needs, event monotonicity.
- Integration: transactions and conflict resolution, scheduler retries, LLM malformed output, database crash/restart.
- Replay: state hashes at checkpoints equal when using recorded decisions and seeds.
- Load: 8, then 20 agents; simulated month; stress conversations; backpressure and OOM behavior.
- Security: prompt-injection fixture, cross-agent memory leak, unauthorized admin requests, malicious visitor content.

## 12. Decisions to confirm before coding

1. Host OS (Windows, Ubuntu, Bazzite or other) and CPU model.
2. Fully offline requirement versus optional paid reflection.
3. Public observers or local-only v1.
4. Visual style and whether assets may use permissively licensed packs.
5. License for code if the GitHub repository may eventually become public.
