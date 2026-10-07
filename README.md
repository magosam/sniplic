<div align="center">
  <h1>🎬 Sniplic Core</h1>
  <p><strong>A blazing-fast, agnostic Non-Linear Video Editing (NLE) engine built from scratch in Rust.</strong></p>

  [![Version](https://img.shields.io/badge/version-0.2.351-blue.svg)]()
  [![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org)
  [![Node.js](https://img.shields.io/badge/Node.js-Native_Bindings-green.svg)](https://nodejs.org)
  [![License](https://img.shields.io/badge/license-MIT-purple.svg)]()
</div>

<br/>

## 📖 The "Why"
Building video editors, automated rendering pipelines, or timeline-based media applications is incredibly hard. Managing timeline state, audio/video synchronization, caching heuristics, and FFmpeg pipelines usually results in deeply coupled, hard-to-maintain code.

After building a full-fledged video editor from scratch, I decided to decouple the **entire engine** and open-source it. **Sniplic Core** extracts all the heavy lifting into a pure, predictable, and memory-safe Rust library, exposing a beautiful API for both Systems Programming (Rust) and the Web Ecosystem (Node.js/TypeScript).

Whether you are building the next web-based video editor, an automated TikTok/Shorts generator, or an AI video pipeline, Sniplic provides the foundation.

---

## ✨ Features

- ⏱️ **Frame-Accurate Playback Engine:** Strict hardware-clock synchronization using Tokio to eliminate jitter, drift, and ensure precise NLE timeline scrubbing.
- 🎞️ **Rich NLE Timeline Model:** Advanced abstractions for Tracks, Clips, Media Pools, Gapless execution, Ripple edits, and Trimming.
- 🤖 **Local AI Subtitling:** Built-in ONNX Runtime bindings optimized for Parakeet TDT to perform blazing-fast, offline audio transcription and word-level timestamp generation.
- 🚀 **Zero-Cost Node.js Bindings:** Seamless integration with V8 via **NAPI-RS**. Control the Rust engine directly from TypeScript with full autocomplete and native performance.
- 📦 **Agnostic & Headless:** 100% decoupled from any UI framework (Tauri, Electron, React). It runs anywhere Rust runs.

---

## 🏗️ Architecture Layers

Sniplic is organized as a Cargo Workspace with two main layers:
1. `sniplic-core`: The pure Rust library.
2. `bindings/node`: The NAPI-RS bridge exporting the engine to JavaScript.

---

## 💻 Getting Started (Node.js / TypeScript)

The fastest way to use Sniplic is via the native Node.js bindings. Ideal for backend automation or Electron/Web-based editors.

### Installation
*(Assuming the package is published or built locally)*
```bash
npm install sniplic
```

### Usage
```typescript
import { SniplicEngine } from 'sniplic';

// 1. Initialize a new blazing-fast project
const engine = new SniplicEngine("My Awesome Video");

// 2. Manipulate the timeline (Example mapping)
console.log(`Initialized: ${engine.getProjectName()}`);
console.log(`Tracks available: ${engine.getTrackCount()}`);

// 3. Save the project structure safely
engine.saveProject("./project_metadata.json");
```

*(Note: To compile the bindings locally from source, navigate to `bindings/node`, run `npm install`, and then `npm run build`).*

---

## 🦀 Getting Started (Rust)

For maximum performance and system-level integration, use the core library directly.

### Installation
Add the dependency to your `Cargo.toml`:
```toml
[dependencies]
sniplic-core = { git = "https://github.com/your-username/sniplic.git", version = "0.2.351" }
```

### Usage
```rust
use sniplic_core::core::project::Project;

#[tokio::main]
async fn main() {
    // Instantiate the Project Model
    let mut project = Project::new("My Rust Video".to_string());
    
    // The media pool, tracks, and timelines are fully accessible
    println!("Project: {}", project.metadata.name);
    println!("Total Tracks: {}", project.tracks.len());

    // Save project
    project.save_to_file("my_rust_project.json").unwrap();
}
```

---

## 🤝 Contributing
This engine is a labor of love and a gift to the open-source community. If you are passionate about Video Engineering, FFmpeg, Rust, or AI, your pull requests are highly welcome! 

Areas looking for contributors:
- Expanding the Node.js API surface (mapping more Rust methods to `#[napi]`).
- WebAssembly (WASM) bindings support.
- Advanced FFmpeg hardware acceleration configurations.

## 📄 License
This project is licensed under the MIT License - see the LICENSE file for details.
