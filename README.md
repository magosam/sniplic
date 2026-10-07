<div align="center">

# 🎬 Sniplic Engine

**The next-generation, headless Non-Linear Video Editing (NLE) engine.**  
Built in Rust for absolute memory safety, blinding speed, and frame-accurate precision.

[![Rust](https://img.shields.io/badge/Rust-1.70%2B-FA4E30?style=for-the-badge&logo=rust)](https://rust-lang.org)
[![Node.js](https://img.shields.io/badge/Node.js-NAPI--RS-339933?style=for-the-badge&logo=node.js)](https://nodejs.org)
[![FFmpeg](https://img.shields.io/badge/Powered_by-FFmpeg-007808?style=for-the-badge&logo=ffmpeg)]()
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=for-the-badge)]()

*Sniplic provides a unified, zero-overhead foundation for building desktop editors, web-based rendering pipelines, and automated social media generators.*

</div>

---

## ✨ Features

- ⏱️ **Frame-Accurate Clock:** Uses a strict u64 frame-based timeline. Zero floating-point drift, pristine scrubbing, and hardware-synchronized Tokio playback.
- 🧩 **Headless & Agnostic:** Runs anywhere. Build your UI in React, Vue, Tauri, Bevy, or use it as a silent backend worker for automated rendering.
- 🎨 **Advanced Compositing:** Native support for CSS-like visual filters, affine transforms, 3D LUTs, and gapless/ripple timeline behaviors.
- 🎛️ **Audio DSP & EQ:** Built-in Equalizers (Bass, Mid, Treble) and Noise Reduction powered by FFmpeg's fftdn and nequalizer.
- 🤖 **Local AI Subtitles:** Deeply integrated ONNX Parakeet TDT for blazing-fast, offline audio transcription and word-level timestamp generation.
- ⏪ **Native Undo/Redo:** In-memory transactional history stacks. Time-travel through your project states instantly without massive JSON overhead.

---

## 🏗️ Architecture

Sniplic is designed as a modular Cargo Workspace, cleanly separating the intensive Rust computation from your application's UI layer.

\\\mermaid
graph TD
    subgraph Frontend Ecosystem
        NODE[Node.js / TypeScript]
        TAURI[Tauri / Rust Native]
    end

    subgraph Sniplic NLE Engine
        NAPI[NAPI-RS Bindings]
        CORE[Sniplic Core]
        TL[Timeline State & Undo Stack]
        DSP[Audio DSP & Parakeet AI]
        FFMPEG[FFmpeg Graph Builder]
    end

    NODE -->|Calls| NAPI
    NAPI --> CORE
    TAURI -->|Native Calls| CORE
    
    CORE --> TL
    CORE --> DSP
    CORE --> FFMPEG
    
    FFMPEG -->|Renders| OUT[Final .mp4 / Proxy]
\\\

---

## 🚀 Getting Started

Sniplic can be consumed dynamically via Node.js or statically as a Rust crate. Choose your ecosystem below.

### 🟢 Node.js / TypeScript (NPM)
Perfect for Next.js servers, Electron apps, or Node CLI tools.

**Installation:**
\\\ash
npm install sniplic-node
\\\

**Quick Start:**
\\\	ypescript
import { SniplicEngine } from 'sniplic-node';

async function main() {
    // 1. Initialize Engine & Canvas
    const engine = new SniplicEngine("TikTok Generator");
    await engine.setProjectConfig(1080, 1920, 60.0); // 9:16 Aspect Ratio

    // 2. Import & Build Timeline
    await engine.importMedia("./raw_footage.mp4");
    await engine.addClipsBatch(["med_1234"], 0, null, true, null);

    // 3. Apply Filters & Transitions
    await engine.setClipTransition("clip_1234", "fade", JSON.stringify({ duration_frames: 30 }));
    await engine.setClipFilter("clip_1234", JSON.stringify({ brightness: 1.1, contrast: 1.05 }));

    // 4. Render with Real-Time Progress
    const settings = { video_codec: "libx264", audio_codec: "aac", bitrate_kbps: 8000 };
    engine.exportProject(JSON.stringify(settings), (err, progress) => {
        if (err) throw err;
        console.log(\[\]: \% \);
    });
}
\\\

### 🦀 Rust (Cargo)
Perfect for native high-performance apps like Tauri or Bevy.

**Installation:**
\\\	oml
[dependencies]
sniplic-core = { git = "https://github.com/sam44cordeiro/sniplic.git", branch = "main" }
\\\

**Quick Start:**
\\\ust
use sniplic_core::core::project::Project;
use sniplic_core::playback::engine::PlaybackEngine;

#[tokio::main]
async fn main() {
    let mut project = Project::new("My Native NLE".into());
    let engine = PlaybackEngine::new();

    // The engine streams tick events perfectly synchronized
    engine.set_on_frame_update(move |event| {
        println!("Needle moved to frame: {} (Playing: {})", event.frame, event.is_playing);
    }).await;

    engine.play().await;
}
\\\

---

## 🤝 Contributing

Sniplic is an open-source labor of love pushing the boundaries of video engineering. Contributions are welcome! 
If you want to add new FFmpeg ilter_complex parsers, improve the WebAssembly (WASM) targets, or optimize the NAPI bridge, please open a PR.

1. Fork the Project
2. Create your Feature Branch (\git checkout -b feat/AmazingFeature\)
3. Commit your Changes (\git commit -m 'feat: add some AmazingFeature'\)
4. Push to the Branch (\git push origin feat/AmazingFeature\)
5. Open a Pull Request

---

<div align="center">
  <p>Built with ❤️ by Sam Cordeiro and the Open Source Community.</p>
</div>
