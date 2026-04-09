# DoppelArm

A Windows desktop application for controlling and visualizing two SO-ARM100 robotic arms in a leader/follower teleoperation setup. Move the leader arm by hand and watch the follower mirror your motions in real time, with a live 3D view of both arms.

![Stack](https://img.shields.io/badge/Tauri-v2-FFC131) ![Stack](https://img.shields.io/badge/SvelteKit-2-FF3E00) ![Stack](https://img.shields.io/badge/Rust-2021-DEA584) ![Platform](https://img.shields.io/badge/Platform-Windows-0078D6)

## Features

- **Motor setup wizard** — assign unique IDs (1-6) to each servo with proper EEPROM commit
- **Live 3D visualization** — both arms rendered side-by-side with Three.js, joint positions stream at 4-50Hz
- **Leader/follower mirroring** — move the leader physically, the follower tracks in real time
- **Calibration** — capture a reference pose and compute per-joint offsets so the two arms align
- **Per-arm joint limits** — software clamping to protect against unsafe positions (e.g. gripper range)
- **Software unwrap** — handles the 0°/360° wrap-around so wrist motions across the boundary stay smooth
- **Recording & playback** — capture teleoperation sessions to JSON and play them back
- **Inverse kinematics** — CCD solver in the frontend (drag-target, work-in-progress UI)
- **Diagnostics** — multi-baud-rate port scan, bus ID scan, raw protocol error dumps

## Hardware

- 2× **[SO-ARM100](https://github.com/TheRobotStudio/SO-ARM100)** robotic arms (one as leader, one as follower)
  - Leader has gears removed from its motors so it can be moved freely by hand
  - Follower has gears installed and is driven by the software
- 12× **Feetech STS3215** smart serial servos (6 per arm) — communicates via half-duplex TTL at 1Mbps
- 2× USB-to-serial adapters (CH340/CP210x) connected to the Waveshare servo controller boards
- Windows PC

## Tech stack

- **Backend**: Rust 2021, Tauri v2, custom Feetech STS3215 protocol implementation over the `serialport` crate
- **Frontend**: SvelteKit 2, Svelte 5, Threlte 8 (Three.js for Svelte), TypeScript, Vite 6
- **3D**: Three.js scene graph driven by Svelte stores; FK via nested rotation groups, CCD IK in TypeScript

## Project structure

```
DoppelArm/
├── src-tauri/                # Rust backend
│   ├── src/
│   │   ├── serial/           # STS3215 protocol, port enumeration
│   │   ├── arm/              # Controller, mirror loop, joint config
│   │   ├── commands/         # Tauri IPC handlers
│   │   ├── calibration.rs    # Offset computation, save/load
│   │   ├── recording.rs      # Frame capture, playback
│   │   └── state.rs          # Shared AppState
│   ├── tauri.conf.json
│   └── Cargo.toml
├── src/                      # SvelteKit frontend
│   ├── lib/
│   │   ├── components/
│   │   │   ├── ArmPane.svelte           # One arm: 3D view + joint sliders
│   │   │   ├── Toolbar.svelte           # Top bar (mirror, record, calibrate)
│   │   │   ├── scene/                   # Threlte 3D components
│   │   │   ├── controls/                # Joint sliders, mirror toggle
│   │   │   ├── setup/                   # Motor setup wizard modal
│   │   │   └── calibration/             # Calibration modal
│   │   ├── stores/                      # Connection, joints, recording state
│   │   ├── tauri/commands.ts            # Typed Tauri IPC wrappers
│   │   ├── ik/solver.ts                 # CCD IK
│   │   └── utils/                       # FK math, arm config
│   └── routes/+page.svelte              # Single-page split-pane layout
└── package.json
```

## Setup

### Prerequisites

- **Rust** 1.80+ (`rustup default stable`)
- **Node.js** 20+
- **Microsoft Edge WebView2** (already installed on Windows 10/11)
- **Visual Studio Build Tools** with the C++ workload (for Tauri's Windows linker)

### Install

```bash
git clone https://github.com/willherr72/DoppelArm.git
cd DoppelArm
npm install
```

### Run in dev mode

```bash
npx tauri dev
```

The first run takes a few minutes as Cargo compiles the Tauri dependency tree. After that, hot reload works for both the frontend and the Rust backend.

### Build for release

```bash
npx tauri build
```

Output is in `src-tauri/target/release/`.

## First-run workflow

The first time you connect a fresh SO-ARM100, all 12 motors come from the factory with the same default ID (1) and have to be programmed individually. The setup is one-time — IDs persist in EEPROM.

### 1. Configure motor IDs (per arm)

1. Plug the controller board into your PC and power it
2. **Disconnect the daisy chain** — unplug the 3-pin cables between motors. Connect only the **shoulder pan** motor to the board.
3. In the app, select the COM port (don't click Connect)
4. Click **Setup IDs** → confirm the motor → click **Configure as ID 1**
5. Disconnect the shoulder pan motor, plug in the **shoulder lift** motor, click **Configure as ID 2**
6. Repeat for elbow, wrist flex, wrist roll, gripper (IDs 3-6)
7. Reconnect the full daisy chain
8. Click **Verify all motors** — should show all 6 IDs found
9. Repeat the whole process for the second arm

### 2. Connect both arms

- Select the leader's COM port → **Connect** (header should show "6 motors")
- Same for the follower

### 3. Calibrate

1. Physically move both arms into the same reference pose (home position works well)
2. Click **Calibrate** in the toolbar → **Capture & Compute**
3. Review the offsets table → **Save calibration**
4. The calibration is persisted to `%APPDATA%\com.doppelarm.app\calibration.json`

### 4. Mirror

- Click **Mirror** in the toolbar
- Move the leader arm by hand — the follower tracks at 50Hz
- Click **Stop Mirror** when done

### 5. Record & playback (optional)

- While mirroring, click **Record** to capture frames
- Click **Stop Rec** to finish
- **Save** writes to `recordings/` and **Play** replays the sequence on the follower

## Notes / gotchas

This project worked through a number of non-obvious hardware quirks. Things to know if you're hacking on the serial layer:

- **STS3215 EEPROM writes need an unlock/write/relock sequence** to commit to flash. The lock register is at address **55** (not 48 — that's `TORQUE_LIMIT`). Without re-locking, writes only land in the RAM mirror and are lost on power cycle.
- **Half-duplex protocol parsing** — different USB-to-serial adapters behave differently. Some echo TX bytes back, some don't, and some lose the first byte of the response on the TX→RX direction switch. The parser scans for `FF FF`, single-`FF`, or no header at all and validates by checksum.
- **Mirror loop must run on a `std::thread`**, not `tokio::task::spawn_blocking`. Sync Tauri commands don't have a Tokio runtime context, and `spawn_blocking` will panic with "no reactor running".
- **Channel sends across the FFI boundary** are wrapped in `catch_unwind` because a Rust panic in the WebView2 callback aborts the process (`STATUS_STACK_BUFFER_OVERRUN`).
- **Calibration is saved to `%APPDATA%`**, not the project directory, because Tauri's dev watcher would otherwise see the file and trigger a rebuild loop.
- **Per-arm joint ID maps** — the leader and follower can have different physical wirings (e.g. wrist roll motor at ID 6 vs ID 5). The kinematic index → servo ID mapping is configurable per arm role.
- **Joint wrap-around** — when a joint crosses the 0°/360° boundary, the encoder reading jumps from 4095 to 0 (or vice versa). The leader read side does software unwrap to track continuous extended positions. The follower clamps at the boundary instead of taking the long way around.

## Status

Working:
- Motor setup, calibration, mirroring, recording, playback
- 3D visualization, joint sliders, diagnostics, port scan

Partial / TODO:
- IK solver works but UI for picking targets is rough
- Follower can't currently track across the wrap-around point (would need multi-loop mode on the servos)
- Cross-platform support (currently Windows-only)
- Bundled installer

## Credits

Built with [Tauri](https://tauri.app/), [SvelteKit](https://kit.svelte.dev/), [Threlte](https://threlte.xyz/), and the [SO-ARM100](https://github.com/TheRobotStudio/SO-ARM100) open-source robotic arm. Serial protocol implementation references [matthieuvigne/STS_servos](https://github.com/matthieuvigne/STS_servos) and the Waveshare ST3215 wiki.
