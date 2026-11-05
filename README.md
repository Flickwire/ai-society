# AI Society — Native Rust Simulation

An offline-first autonomous society in a local native application, with a Bevy 3D viewer, renderer-free headless runner, accelerated virtual time, persistent memory and independently configurable inference providers.

**Status:** implementation/deployment plans only; no runtime has been written.

## Architectural decisions

- **Entire application in Rust.** No TypeScript, JavaScript runtime, Node.js services, browser frontend, or web deployment.
- **Bevy native viewer** for exploration, inspection, time controls and playback.
- **Pure Rust simulation core**, independent of Bevy rendering, wall-clock ticks and GPU resources.
- **Separate headless executable** for maximum-speed experiments, benchmarks and unattended runs.
- **SQLite/WAL** with append-only event history, snapshots, migrations and experiment branching.
- **Swappable inference** through a Rust provider trait: local llama.cpp via OpenAI-compatible HTTP, or opt-in commercial APIs. Different models may serve routine, dialogue and reflection roles.
- **Single-machine only** initially; no external-facing server, public observer or multiplayer requirement.
- **Hardware target:** RX 7700 XT (12 GB VRAM), 64 GB RAM. OS/CPU to confirm.

## Workspace

```text
crates/
  society-domain/       # pure IDs, actions, state and rules
  society-sim/          # virtual clock, deterministic event reducer
  society-cognition/    # perception, goals, memory and reflection
  society-inference/    # provider abstractions, transport, budgets
  society-storage/      # SQLite and snapshot/event storage
  society-view/         # Bevy 3D rendering and inspector
  society-app/          # native graphical executable
  society-headless/     # renderer-free CLI executable
  society-bench/        # load and inference benchmarks
assets/
migrations/
experiments/
docs/
```

## Plans

- [Implementation](docs/IMPLEMENTATION.md)
- [Deployment and operations](docs/DEPLOYMENT.md)
- [Cost and performance benchmarking](docs/COSTS-AND-BENCHMARKS.md)

## Core contract

LLMs propose typed actions; the simulation validates and commits them. Renderer state is a projection of the simulation, not authoritative state. Virtual time advances through causally ordered events, with inference backpressure. Replay reuses recorded model outputs for deterministic reconstruction.

## Not in v1

Public hosting, JavaScript stack, networked multiplayer, real-money economy, blockchain, unrestricted agent shell/network access, foundation-model training, and claims of guaranteed emergence or consciousness.
