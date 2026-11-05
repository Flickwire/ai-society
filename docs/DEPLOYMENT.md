# Local native deployment and operations

## 1. Deployment target

One personally owned gaming computer: AMD Radeon RX 7700 XT (12 GB VRAM), 64 GB DRAM. Confirm exact CPU, OS and driver before choosing the inference binary. The application is local-only: no cloud hosting, public endpoints, browser frontend, containers or resident server processes beyond an optional local llama.cpp inference server. Remote inference endpoints are optional outbound HTTP services, not application hosting. Nothing may use employer infrastructure or credentials.

## 2. Binaries and runtime layout

- `society-app`: native Bevy viewer, inspector, controls; hosts or connects in-process to the Rust simulation. Runs with a limited render FPS when local GPU inference is active.
- `society-headless`: pure Rust simulation runner; no renderer, window, GPU graphics initialization or display requirement.
- `society-bench`: benchmark harness for local/remote inference, action validity and virtual-time throughput.
- `llama-server`: separately launched local inference service on loopback. Not a per-agent process.
- SQLite save DB plus versioned event logs, snapshots, per-experiment directories and optional detailed traces.

Default directory layout should use platform-specific user data/config paths (e.g. via `directories` crate), with selectable portable mode for easy backups. Keep model weights in a separate configurable folder and never include them in Git.

## 3. Install and build

Install stable Rust toolchain, matching native linker and relevant OS graphics dependencies; verify Bevy runs with the RX 7700 XT. Build `cargo build --release -p society-app -p society-headless -p society-bench`. Add native packaging only after a working release build; no Node, npm, pnpm, JS runtime, database server or Docker required.

Create first-run configuration file with no secrets and generate a world seed. SQLite migrations run on startup with pre-migration snapshot. Run `society-headless doctor` (to implement) to check DB permissions, free disk space, model endpoint capabilities, GPU memory and API config. Provide a safe `--offline` switch that forbids any remote inference traffic and requires local inference.

## 4. AMD inference setup

1. Confirm OS/CPU, RX 7700 XT driver and available 12 GB VRAM.
2. Choose a compatible llama.cpp build: Vulkan for simple cross-platform setup; compare ROCm/HIP using a supported AMD distribution/driver combination. Do not use unsupported architecture spoofing.
3. Download a verified, appropriately licensed Qwen3.5-9B Q4_K_M GGUF from a trusted publisher; record hash and chat-template version.
4. Start `llama-server` bound to `127.0.0.1` on a documented port with bounded 8K context, initially one inference slot, max completion length and timeouts. Avoid exposing it on LAN or public interfaces.
5. Run representative 2,000-input/300-output action probes, track prefill/decode speed and peak VRAM. Try Q5_K_M and smaller 4B models. Test two slots only if VRAM headroom and latency justify it.
6. Compare Vulkan and ROCm end-to-end under equal prompts/settings. Keep the known-good runtime/model as a rollback option.

Allow either the app to spawn/manage a configured local llama-server child process (explicit user path, constrained flags, clean shutdown) or use an already-running endpoint. Do not download and execute binaries silently.

## 5. Provider configuration

Use a versioned TOML file with named inference profiles and model roles, e.g.:

```toml
[inference]
active_profile = "local"
offline = true

[inference.profiles.local]
kind = "openai_compatible"
base_url = "http://127.0.0.1:8080/v1"
api_key_env = ""

[inference.profiles.remote]
kind = "openai_compatible"
base_url = "https://YOUR-PROVIDER-ENDPOINT/v1"
api_key_env = "AI_SOCIETY_REMOTE_API_KEY"

[inference.roles.routine]
profile = "local"
model = "qwen-local"
max_input_tokens = 2000
max_output_tokens = 300
max_concurrent = 1

[inference.roles.reflection]
profile = "local"
model = "qwen-local"
max_input_tokens = 4000
max_output_tokens = 700
```

This is an illustrative schema, not an implemented parser. Add capability validation, aliases for models and explicit profile enablement. Remote use requires setting `offline=false` and an explicit profile change. Do not automatically send any prompt to a different trust boundary on failure. Optional roles can be individually remote, allowing local routine decisions and hosted reflections. Token/cost budgets and a provider fail-closed policy are mandatory. Read keys only from environment or OS credential store, never commit them.

## 6. Modes of operation

Native UI: ordinary 3D viewer with selectable time multiplier and manual pause. Headless: `society-headless run --world WORLD --mode fast-forward --until-virtual-days 30` (planned CLI), with periodic progress logs and checkpoints. Replay: `society-app replay --experiment ID` or headless verification. CLI progress reports wall time, virtual time, acceleration, model throughput, queue backlog and estimated spending.

Fast-forward advances to discrete events without rendering. Capped acceleration slows rather than dropping causal decisions when inference lags. Viewer FPS should be limited to 15–30 during inference-heavy sessions; consider stopping GUI entirely for overnight simulation.

## 7. Storage and resilience

SQLite WAL with one writer, transaction boundaries across state and event append, idempotent decision IDs, versioned migrations, regular WAL checkpoints and tested restore. Back up DB (using SQLite online backup API or safe checkpoint/backup flow), immutable event history and verified compressed snapshots. Keep last several snapshots and nightly off-device encrypted backups if the world becomes valuable. Do not copy a live DB file blindly and assume consistency.

On startup, validate DB schema and latest snapshot hash, replay committed events, identify incomplete decisions, and either safely resume them or invalidate stale responses. On shutdown, stop new model requests, wait or cancel bounded pending work, commit checkpoint and terminate managed child processes. Crash recovery cannot duplicate trades.

## 8. Performance/resource limits

- Target GPU VRAM: leave roughly 1–2 GB headroom for the desktop/Bevy renderer; actual limit determined by measurements.
- Initial local model: Qwen3.5-9B Q4 with 8K context, one slot; test parallelism rather than assume scaling.
- Initial society: 8 agents averaging 8 decisions per virtual hour.
- Limit open DB statements, task queues, per-agent wake-ups and prompt/conversation lengths.
- Configure maximum output, request timeout and per-virtual-day budget by role.
- Cap renderer FPS; provide reduced graphics mode and full headless mode.
- Use disk retention policy for detailed traces, but never silently truncate canonical event history.

## 9. Security

Loopback-only inference by default; no inbound network listener required for the app. If remote inference is enabled, require HTTPS, explicit provider hostname, configured key and privacy notification in the UI. Never send API keys, entire DB contents or other agents' private memories as model prompt context. Validate untrusted dialogue and model responses; agents cannot execute OS commands, fetch arbitrary URLs or edit save files. Admin controls are local to the desktop app. Logs must redact credentials and allow configurable retention.

## 10. Verification checklist

- [ ] Confirm OS/CPU and compatible AMD driver/runtime.
- [ ] Build Rust workspace and native app; no JS toolchain.
- [ ] Start local inference on loopback; verify format/capabilities and VRAM.
- [ ] Run deterministic no-inference headless test.
- [ ] Run eight-agent inference endurance test; record actual acceleration.
- [ ] Restart after clean and forced shutdown; verify state hashes and no duplicate events.
- [ ] Replay stored outputs and compare checkpoints.
- [ ] Switch from local to a test remote endpoint via TOML only, then back.
- [ ] Test remote cost exhaustion and offline mode: no unexpected network calls.
- [ ] Validate backup/restore and safe schema migration.

## 11. Failure policies

| Failure | Response |
|---|---|
 GPU OOM | Reduce slots/context or switch to smaller local model; no world mutation from failed inference |
 Model timeout | Continue safe timed task or rest; bounded retry |
 Invalid JSON/action | Reject with diagnostic; never write invalid state |
 Conflicting actions | Stable serial commit, version/precondition check and explicit failure |
 Remote outage | Fail closed by default; fallback only to explicitly permitted profile |
 API budget hit | Stop paid inference and optionally use configured local model |
 SQLite error | Pause authoritative writer and surface recoverable error |
 Replay hash mismatch | Stop at first divergence and preserve diagnostics |
 Viewer crash | Headless runner unaffected; saved state remains transactional |

## 12. Distribution later

Provide Windows and Linux release packages for a single native executable plus assets and a separate headless tool. Model weights and inference runtime remain optional, separately verified downloads due to size/licensing. No application updater or auto-execution of downloaded model runtimes without user opt-in.
