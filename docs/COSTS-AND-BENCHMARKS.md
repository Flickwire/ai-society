# Cost, acceleration and measurement model

## Assumptions

8 agents × 8 decisions per virtual hour = 64 decisions/virtual hour. Each decision averages 2,000 input tokens plus 300 generated tokens. A virtual day therefore requires 1,536 decisions, 3.072 million input tokens and 460,800 generated tokens. This excludes reflection, media and tool use.

The RX 7700 XT has 12 GB VRAM and up to 432 GB/s physical memory bandwidth, not 16 GB VRAM. 64 GB DRAM helps for simulation and offloading but does not have VRAM-like GPU throughput.

## Acceleration formula

For `N` agents, `d` decisions per simulated agent-hour, prompt `p`, output `o`, aggregate prefill rate `P`, aggregate generation rate `G`:

`real_seconds_per_sim_hour = N*d*(p/P + o/G + per_request_overhead_seconds)`

`maximum_acceleration = 3600 / real_seconds_per_sim_hour`

These simplified rates apply to a serialized workload, ignore retrieval/simulation overhead and cannot predict parallel serving automatically. Measure aggregate service throughput under realistic concurrency and use the observed end-to-end response times for capacity planning.

| Scenario | Prompt tok/s | Output tok/s | Estimated acceleration (zero extra overhead) |
|---|---:|---:|---:|
| Conservative | 400 | 30 | 3.75x |
| Middle | 600 | 40 | 5.54x |
| Optimistic | 900 | 55 | 7.34x |

At half the decision frequency, the theoretical multiplier doubles. Context caching can reduce prefill cost and latency, but do not count on it before benchmarking. Conversations can create bursts far above the average.

## Benchmark protocol

1. Record exact GPU, CPU, OS, driver, runtime commit, backend, GGUF source hash, quantisation, context, slot count and batch settings.
2. Use 50 representative observations and action schemas; each request ~2K input and capped 300 output tokens.
3. Warm model and run serial for 100 requests. Track first-token latency, total duration, effective prompt and completion tokens/s, token counts and valid-action rate.
4. Repeat with 2 and 4 concurrent sequences. Track total throughput, per-agent p95 latency and GPU memory; reduce concurrency if OOM or latency becomes unacceptable.
5. Repeat at 8K and 16K max contexts, and with the 4B and 9B candidate models.
6. Run a one-hour real-time endurance test and a one-day virtual fast-forward test; compute `virtual_elapsed / wall_elapsed` using actual event logs.
7. Include simulated conversations, multiple simultaneous market actions, and idle/sleep cycles; verify state invariants.
8. Compare Vulkan and ROCm/HIP on same dataset and settings. Publish local benchmark JSON into `benchmarks/` only after scrubbing personal content.

## Cloud inference reference estimate

At the baseline 30 days of virtual time each month: 46,080 decisions, 92.16M input, 13.824M output tokens. Indicative November 2026 provider list-price calculations (USD, uncached, without reflection or fees):

| Model | Input $/M | Output $/M | Cost for 30 virtual days |
|---|---:|---:|---:|
| Qwen3.5-9B (DeepInfra) | 0.10 | 0.15 | $11.29 |
| Qwen3.5-35B-A3B (DeepInfra) | 0.14 | 1.00 | $26.73 |
| Qwen3-235B-A22B-Instruct-2507 (DeepInfra) | 0.09 | 0.55 | $15.90 |
| GPT-6.1 Sol (OpenAI) | 2.00 | 10.00 | $322.56 |

Note: the 235B price above is derived from the provider's published list price; provider availability and price may change. These costs depend on **simulated days completed**, not wall-clock runtime. Faster simulation consumes a month's equivalent of tokens faster in real time.

## Electricity

For illustration only, at £0.25/kWh:

| Whole-system draw | 8 hours | 24 hours | Continuous 30-day month |
|---|---:|---:|---:|
| 250 W | £0.50 | £1.50 | £45 |
| 450 W | £0.90 | £2.70 | £81 |

Measure actual plug-level power during inference, not just GPU board power. Hosted APIs may cost less than electricity if running continuously; local inference gives privacy, predictable marginal token cost and unmetered experimentation.

## Budget limits

- Local mode: no token billing; monitor electricity, latency and thermal headroom.
- Hybrid mode: commercial fallback disabled by default; explicit per-day/per-month caps, and a fail-closed option.
- If public visitors are eventually enabled, charge inference against visitor interaction and agent budgets; rate-limit conversations and prevent users forcing endless loops.
- Maintain dashboards for tokens per virtual day, cost per successful action, percent rejected actions, inference wait share and actual acceleration multiplier.

## Sources to recheck before deployment

- AMD RX 7700 XT specifications: https://www.amd.com/en/products/graphics/desktops/radeon/7000-series/amd-radeon-rx-7700-xt.html
- AMD ROCm GPU support: https://rocm.docs.amd.com/projects/install-on-linux/en/docs-7.2.2/reference/system-requirements.html
- AMD llama.cpp guide: https://rocm.docs.amd.com/projects/ai-ecosystem/en/latest/inference/llamacpp.html
- llama.cpp project: https://github.com/ggml-org/llama.cpp
- DeepInfra prices: https://deepinfra.com/pricing
- OpenAI model rates: https://developers.openai.com/api/docs/models
