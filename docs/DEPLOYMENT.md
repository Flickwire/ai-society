# Deployment and operations plan

## 1. Target machine

Personal gaming computer, AMD Radeon RX 7700 XT **12 GB** VRAM, 64 GB RAM. CPU and operating system remain to be confirmed. No employer inference service, infrastructure, tokens or development credentials may be used. Keep all project data and model downloads under user-controlled storage.

## 2. Environment tiers

**Local development:** Docker Compose for PostgreSQL + pgvector, Redis (once required), world-server, worker and web UI. Native `llama-server` on host for reliable AMD GPU access. Bind database and admin ports to localhost. Inference endpoint defaults to `http://127.0.0.1:<port>/v1` on the host; inside containers use a correctly scoped host-gateway address, not `localhost`. No public ingress.

**Personal LAN:** same services; optional reverse proxy with TLS and authenticated viewer. Admin endpoint restricted to VPN or local network. Do not expose inference directly.

**Public read-only later:** deploy website/static assets separately and a hardened world-stream gateway, with API authentication, CORS allowlist, rate limits, service-level traffic caps and privacy review. Maintain local operator control and a kill switch.

## 3. GPU runtime path

1. Verify OS and display driver, GPU enumeration and 12 GB VRAM. Check compatibility against the current AMD ROCm matrix.
2. Install current compatible `llama.cpp` build with Vulkan backend for the simplest first run.
3. Select Qwen3.5-9B Q4_K_M GGUF from a trusted publisher; verify model license, checksum and chat template. Never commit model weights.
4. Configure `llama-server` for bounded 8K context, 1–2 slots, output token cap, localhost binding and logs. Reserve headroom for KV caches and desktop rendering. Begin with one slot and add a second only after measuring VRAM.
5. Run 2K-prompt/300-output representative benchmark with 1/2/4 concurrent requests. Record prefill and aggregate output throughput, p50/p95 latency and peak VRAM.
6. Compare ROCm/HIP build when supported on the selected OS. Keep whichever backend is faster and stable under a 1-hour load test.
7. Benchmark Q5_K_M and smaller 4B candidates; retain a validated known-good Q4 baseline.

AMD includes RX 7700 XT (`gfx1101`) in current ROCm compatibility information. Vulkan and ROCm both support quantised llama.cpp inference, but throughput and OS compatibility must be measured. Avoid relying on undocumented GPU architecture overrides.

## 4. Configuration and secrets

Use `.env.example` with placeholders only. Required values include `DATABASE_URL`, `WORLD_SEED`, `SIMULATION_MODE`, `MAX_VIRTUAL_SPEED`, `INFERENCE_BASE_URL`, `INFERENCE_MODEL`, `MAX_PARALLEL_REQUESTS`, `MAX_INPUT_TOKENS`, `MAX_OUTPUT_TOKENS`, `INFERENCE_DAILY_USD_CAP`, `LOG_RETENTION_DAYS`, `ADMIN_AUTH_SECRET` and optional `CLOUD_API_KEY`. Never commit `.env`, private database dumps, user uploads, API keys, logs containing sensitive content or GGUF weights.

Use personal commercial API credentials only when cloud fallback is explicitly enabled. Set a total spend cap at both application level and provider account level where supported. Make fallback fail closed when cost is unknown.

## 5. Operations

- Startup order: database health -> migrations -> world state recovery -> scheduler -> web viewer. Start the inference server before enabling autonomous workers.
- Shutdown: stop accepting new decisions, drain or persist pending events, record snapshot and world event offset, then stop services.
- Health checks: DB ping, event lag, worker heartbeat, model endpoint readiness, GPU memory headroom, pending request count and budget remainder.
- Alerts: memory over threshold, repeated invalid model output, inference timeout storm, event lag beyond configured maximum, failed snapshot, failed backups.
- Recovery: restore last verified snapshot then replay append-only event log. Do not duplicate pending idempotent action IDs.
- Backups: nightly compressed database dump and snapshots to a second disk; weekly off-device encrypted copies if the world becomes valuable. Test restore monthly.
- Updates: pin runtime and model versions; stage migration against a copy; snapshot before deployment; keep previous image/build for rollback.
- Model logs: redact sensitive visitor content, set retention policy, exclude secrets, never send private traces to telemetry by default.

## 6. Headless acceleration

The simulation must run without 3D rendering. Modes: `realtime`, `capped`, `fast_forward`, and `replay`. In fast-forward, event scheduler advances to the next meaningful virtual event; LLM calls impose backpressure. Store both monotonic wall-clock timing and independent virtual event timestamps. If throughput falls below a capped multiplier, slow the virtual clock instead of dropping important decisions. Provide pause, resume and safe checkpointing.

## 7. Suggested performance and resource budgets

- GPU VRAM target: remain below ~10.5 GB peak during normal operation; tune per driver and desktop needs.
- Initial context: 8,192 tokens per sequence, later benchmark 16K.
- Resident count: 8, scale to 20 only after a stable baseline.
- Agent decisions: target average 8 per agent per virtual hour; event spikes allowed within global bounds.
- Database: start with 10–20 GB of dedicated SSD space and monitor growth.
- Logs: keep 7–14 days detailed traces; compact historical metrics and event records deliberately, not implicitly.
- Snapshot: every virtual day or fixed event count, plus before upgrades and experiment forks.

## 8. Personal deployment checklist

- [ ] Confirm OS/CPU and GPU drivers.
- [ ] Verify no work accounts, endpoints, keys or code are configured.
- [ ] Run chosen local inference model and record baseline throughput.
- [ ] Provision persistent Docker volumes; verify backups and restore.
- [ ] Configure local-only bind addresses and admin authentication.
- [ ] Run schema migrations and seed the town.
- [ ] Verify deterministic 24-hour no-model test.
- [ ] Run 8-agent accelerated test with metric collection.
- [ ] Test Ctrl+C/restart, crash recovery and replay checksums.
- [ ] Only then consider LAN access or public spectator deployment.

## 9. Failure modes

| Failure | Desired behavior |
|---|---|
| Model OOM | Retry with fewer slots/shorter context; no world corruption |
| Invalid JSON/action | Reject with reason; limited retry, then safe fallback |
| Model unavailable | Pause decisions or continue timed tasks; never fabricate choices |
| Database disconnect | Stop committing events; queue safely or pause |
| Concurrent conflicting purchase | Optimistic version check and atomic winner |
| Replay mismatch | Halt and report first differing event/state hash |
| Budget exhaustion | Disable paid provider; continue local-only or pause |
| Browser disconnect | Headless world continues normally |

## 10. Optional future cloud deployment

If moving off the gaming PC, host world services and PostgreSQL on a small Linux VPS; use commercial open-weight inference APIs rather than renting a 24/7 GPU initially. Keep optional web frontend on static hosting. Use encrypted backup storage, HTTPS, per-agent inference budgeting and production monitoring. Never treat a consumer chatbot subscription as an unattended API allowance.
