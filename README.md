<div align="center">
  <h1>🎬 Sniplic Core</h1>
  <p><strong>A blazing-fast, agnostic Non-Linear Video Editing (NLE) engine built in Rust.</strong></p>

  [![Version](https://img.shields.io/badge/version-0.2.363-blue.svg)]()
  [![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org)
  [![Node.js](https://img.shields.io/badge/Node.js-Native_Bindings-green.svg)](https://nodejs.org)
  [![License](https://img.shields.io/badge/license-MIT-purple.svg)]()
</div>

<br/>

## Overview

**Sniplic Core** is a highly optimized, memory-safe video editing engine designed to power the next generation of media applications. It abstracts the immense complexity of timeline state management, real-time audio/video synchronization, and FFmpeg command generation into a clean, predictable API.

Whether you are building a modern web-based video editor, a native desktop application, an automated social media video generator, or an AI-driven rendering pipeline, Sniplic provides the robust foundation needed to handle media at scale.

## Core Capabilities

- **Frame-Accurate Engine:** Hardware-clock synchronization using Tokio, utilizing a strict \u64\ frame-based time system to eliminate floating-point drift and ensure pristine scrubbing.
- **Advanced Timeline Architecture:** Deep support for multi-track composition, gapless editing, ripple logic, trimming, splitting, and undo/redo history stacks.
- **High-Performance Node.js Bindings:** Zero-cost integration with V8 via **NAPI-RS**. Control the full Rust engine directly from TypeScript with native performance, fully synchronous callbacks, and non-blocking asynchronous exports.
- **Server-Authoritative Rendering:** Generates complex FFmpeg \ilter_complex\ chains automatically, supporting CSS-like filters, xfade transitions, color LUTs, and affine transforms.
- **DSP Audio Pipeline:** Native injection of Equalizers (Bass, Mid, Treble) and Noise Reduction directly into the engine's rendering graphs.
- **Local AI Subtitling:** Built-in ONNX Runtime bindings optimized for offline audio transcription and word-level timestamp generation.

---

## Getting Started (Node.js / TypeScript)

Sniplic exposes a feature-complete NPM module. It is ideal for Electron backends, Next.js servers, or any Node.js automation pipeline.

### Installation

`ash
npm install sniplic-node
`

### Usage

`	ypescript
import { SniplicEngine } from 'sniplic-node';

// 1. Initialize the NLE Engine
const engine = new SniplicEngine("My Project");

// 2. Set Target Aspect Ratio (e.g., TikTok/Reels)
await engine.setProjectConfig(1080, 1920, 60.0);

// 3. Import Media & Build Timeline
await engine.importMedia("/path/to/video.mp4");
await engine.addClipsBatch(["med_1234"], 0, null, true, null);

// 4. Apply Filters and Audio EQ
await engine.setClipFilter("clip_1234", JSON.stringify({ brightness: 1.2 }));
await engine.applyAudioPlugin("clip_1234", JSON.stringify({ eq_bass: 5.0, denoise: 0.8 }));

// 5. Render Project
engine.exportProject(JSON.stringify(exportSettings), (err, progress) => {
    if (progress.stage === "done") {
        console.log("Render completed!");
    } else {
        console.log(Rendering: %);
    }
});
`

---

## Getting Started (Rust)

For absolute maximum performance and system-level integration (e.g., Tauri, Bevy, Axum), use the core library directly.

### Installation

Add the dependency to your \Cargo.toml\:

`	oml
[dependencies]
sniplic-core = { git = "https://github.com/sam44cordeiro/sniplic.git", version = "0.2.363" }
`

### Usage

`ust
use sniplic_core::core::project::Project;
use sniplic_core::playback::engine::PlaybackEngine;

#[tokio::main]
async fn main() {
    let mut project = Project::new("My Native App".to_string());
    
    // Subscribe to playback events using Tokio broadcast channels
    let engine = PlaybackEngine::new();
    let mut rx = engine.subscribe();

    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            println!("Playhead moved to frame: {}", event.frame);
        }
    });
}
`

---

## Architecture 

Sniplic is organized as a Cargo Workspace with two distinct layers:
1. \src/\: The agnostic, pure Rust core engine (\sniplic-core\).
2. \indings/node/\: The NAPI-RS bridge exporting the engine to JavaScript (\sniplic-node\).

## License
This project is licensed under the MIT License - see the LICENSE file for details.
