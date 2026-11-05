# Local performance, acceleration and inference costs

## 1. Hardware assumptions

AMD Radeon RX 7700 XT = **12 GB VRAM**, up to 432 GB/s memory bandwidth, plus 64 GB system RAM. Exact OS, CPU and power measurements are pending. Shared DRAM is not equivalent to GPU VRAM. Bevy rendering and local inference compete for GPU memory and compute; headless mode removes the renderer load.

## 2. Baseline society workload

8 residents × 8 decisions per virtual hour = 64 requests per virtual hour. Each prompt averages 2,000 input tokens and each decision 300 output tokens. Therefore one virtual day needs 1,536 model calls, 3.072M input and 460,800 output tokens. Reflection, embedding, creative tasks and retries add usage. Token counts must be measured from actual requests after implementation.

## 3. Speed model

With N residents, d decisions per simulated resident-hour, p input tokens per request, o output tokens, aggregate prompt rate P and aggregate output rate G:

`seconds_per_virtual_hour ≈ N*d*(p/P + o/G + request_overhead)`

`maximum_virtual_speed = 3600/seconds_per_virtual_hour`

This is a serial-style planning estimate; concurrent serving needs measured end-to-end aggregate throughput. Do not multiply single-request tok/s by the number of slots.

| Assumed scenario | Prompt tok/s | Output tok/s | Upper bound virtual speed without overhead |
|---|---:|---:|---:|
 Conservative | 400 | 30 | 3.75× |
 Middle | 600 | 40 | 5.54× |
 Optimistic | 900 | 55 | 7.34× |

At four decisions per agent per virtual hour, idealised throughput doubles. Accelerated time does not make inference free: more simulated days per wall hour means more model calls per wall hour. Headless operation primarily removes rendering overhead and simplifies the event clock.

## 4. Model and backend matrix

Candidates: Qwen3.5-9B Q4_K_M baseline, Q5_K_M quality alternative, a suitable 4B model for cheap routine decisions, and 12–14B Q4 only if VRAM permits context/cache headroom. Do not plan 27B+ Q4 fully in 12 GB; CPU offloading may be much slower. Test llama.cpp Vulkan and ROCm/HIP on the actual OS. Use one loaded inference server across all agents, not eight copies. Start one slot at 8K context, then benchmark two and four slots. Quantisation and model licenses must be verified per downloaded artifact.

## 5. Mandatory benchmark protocol

1. Record OS, CPU, driver, GPU, llama.cpp revision, backend, model SHA, quantisation, context, slot count, batch and KV cache settings.
2. Create 50 representative world-observation fixtures with 2K input tokens and <=300 output tokens, including structured action schema.
3. Warm up, then run at least 100 serial requests; measure prompt processing, output throughput, end-to-end p50/p95, action validity, GPU memory and watts.
4. Repeat with 2 and 4 concurrent requests. Calculate aggregate completed decisions/min, not sum of isolated speed readings.
5. Compare 4B/9B Q4/Q5, Vulkan vs ROCm, GUI at 30 FPS and truly headless.
6. Run 1 real hour of full eight-agent activity and compute actual virtual_elapsed/wall_elapsed, retries and event backlog.
7. Run 7 virtual days with one high-conversation scenario and one quiet/idle scenario; test backpressure and GPU OOM.
8. Export anonymised benchmark JSON; preserve model/runtime versions for comparisons.

## 6. Cost calculation: local and remote

If the world completes 30 virtual days, that entails 46,080 routine decisions and roughly 92.16M input plus 13.824M output tokens. Indicative November 2026 per-million-token prices, subject to rechecking:

| Provider/model | Input USD/M | Output USD/M | Baseline 30 virtual days |
|---|---:|---:|---:|
 DeepInfra Qwen3.5-9B | 0.10 | 0.15 | $11.29 |
 DeepInfra Qwen3.5-35B-A3B | 0.14 | 1.00 | $26.73 |
 DeepInfra Qwen3-235B-A22B-Instruct-2507 | 0.09 | 0.55 | $15.90 |
 OpenAI GPT-6.1 Sol | 2.00 | 10.00 | $322.56 |

Prices exclude caching, extra reasoning, model-specific billing rules, web/media tools and provider changes. A mixture of local routine and remote reflection should be metered by role. The same 30 virtual days cost roughly the same number of tokens whether generated in one fast-forward weekend or a calendar month.

Local GPU inference has no per-token API bill but uses electricity. At an assumed £0.25/kWh and whole-system active draw of 250–450 W: 8h costs £0.50–£0.90; continuous 30 days costs £45–£81. Measure at the plug; actual power varies significantly. Hosted inference may be economically better for continuous operation, while local inference offers privacy and unmetered exploration.

## 7. Operational targets

- Track percent time waiting on inference, input/output/cache tokens and actual cost.
- Measure event rate, state hash consistency, queue depth, memory and GPU peak use.
- Hard remote daily/monthly caps; app-side cap plus provider-side account limit.
- Benchmark model quality: valid structured action %, repeated-action loops, goals completed, conversational coherence, memory retrieval, tasks per 1M tokens.
- Select the smallest sufficient model for routine tasks; move only complex planning/reflection to expensive profiles.

## 8. Primary references

- AMD RX 7700 XT specs: https://www.amd.com/en/products/graphics/desktops/radeon/7000-series/amd-radeon-rx-7700-xt.html
- AMD ROCm compatibility: https://rocm.docs.amd.com/projects/install-on-linux/en/docs-7.2.2/reference/system-requirements.html
- AMD llama.cpp guide: https://rocm.docs.amd.com/projects/ai-ecosystem/en/latest/inference/llamacpp.html
- llama.cpp: https://github.com/ggml-org/llama.cpp
- Bevy: https://bevy.org/
- DeepInfra pricing: https://deepinfra.com/pricing
- OpenAI API pricing: https://developers.openai.com/api/docs/pricing
