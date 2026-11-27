# VIctory — Intel Arc B580 fork

An experimental fork of [VIctory](https://github.com/Marvisak/VIctory), focused on getting its Xbox Series title compatibility layer working on **Intel Arc graphics under Linux via Steam Proton**.

The first development target is the **Intel Arc B580 (12 GB, Battlemage)** on Arch Linux. The aim is to make the existing D3D12 host backend behave reliably through Proton/VKD3D and to measure performance before attempting larger architectural changes.

> **Status: experimental, not yet playable.** The current test setup can start the title and produce audio, but the rendered viewport is black. Current logs show major problems with host-pipeline availability and indirect GPU commands. This fork is an active compatibility experiment, not a finished Linux port.

This repository contains **no game executable, game assets, saves, caches, or other copyrighted game data**. You must provide files you are legally entitled to use. This project is not affiliated with or endorsed by any game developer, publisher, Intel, Valve, or Microsoft.

## Target test system

| Component | Current test setup |
|---|---|
| OS | Arch Linux x86_64 |
| Desktop | KDE Plasma on Wayland |
| GPU | Intel Arc B580, 12 GB |
| Kernel graphics driver | `xe` |
| Userspace graphics | Mesa / Vulkan |
| CPU | AMD Ryzen 5 4500 |
| RAM | 48 GB |
| Compatibility layer | Steam Proton Experimental |
| Rust toolchain | 1.98.1 |

These are the current development machine's details, not minimum requirements or a claim that other systems work.

## How the graphics path works

VIctory currently creates a **Windows D3D12 device**. When the Windows-targeted executable runs through Proton, the D3D12 calls are translated by Proton's D3D12-to-Vulkan stack (VKD3D-Proton) and then reach the Linux Vulkan driver.

That means this fork is **not a native Linux/Vulkan backend**. The first goal is to make the existing D3D12 path work correctly on Intel Arc through Proton. A native backend would be a separate, much larger project and is not assumed to be necessary.

See [docs/architecture.md](docs/architecture.md) for the internal architecture.

## Build on Arch Linux

Install the build tools:

```bash
sudo pacman -S --needed git rustup mingw-w64-gcc
```

Clone the repository:

```bash
git clone https://github.com/janwin443/victory-arc-b580.git
cd victory-arc-b580
```

The repository pins its Rust toolchain in `rust-toolchain.toml`. Install the Windows GNU target:

```bash
rustup target add x86_64-pc-windows-gnu
```

Build the workspace:

```bash
CARGO_TARGET_DIR=/mnt/Daten/cargo-target \
cargo build --workspace --release --target x86_64-pc-windows-gnu -F blake3/pure
```

Change `CARGO_TARGET_DIR` to any writable build directory you prefer. The `blake3/pure` feature avoids a MinGW assembler issue encountered on the development machine.

The main executable is produced at:

```text
$CARGO_TARGET_DIR/x86_64-pc-windows-gnu/release/victory.exe
```

## Run through Steam Proton

The current test workflow uses Steam rather than launching Proton directly:

1. Build `victory.exe` as described above.
2. Add `victory.exe` to Steam as a non-Steam game.
3. In its Compatibility settings, force **Proton Experimental**.
4. Set the working directory to the directory containing `victory.exe`.
5. Pass the target executable as an argument to VIctory:

```text
run "Z:\\path\\to\\your\\game.exe" --execute
```

Use the actual Windows-style path as seen from Proton. Keep the target game's files outside this repository. Do not put game files, extracted assets, save data, shader caches, or generated binaries into Git.

The exact launch setup may change as the launcher and Proton integration are investigated.

## Current development priorities

1. **Reliable diagnostics:** distinguish invalid indirect-command layouts, missing clamp resources, scratch-buffer failures, address/alignment failures, and command-recording failures.
2. **Indirect commands:** investigate mesh dispatch (`kind 10`) and other indirect command paths that are currently dropped.
3. **Host pipelines:** understand why required pipelines are unavailable or still compiling when the title needs them.
4. **Rendering correctness:** get a stable, non-black frame before attempting performance tuning.
5. **Performance:** measure CPU preparation, GPU execution, asset streaming, and read-back separately. Do not treat an internal estimate as actual GPU utilization.

Changes should be evidence-driven. Avoid enabling experimental switches or changing multiple subsystems at once: a run should tell us whether a specific change helped.

## CI checks

GitHub Actions runs on pushes to `main` and on pull requests. The workflow checks:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --locked`
- Clippy with the default and all-feature configurations
- `cargo test --workspace --locked`

A passing CI run verifies the configured Windows build checks and tests; it does **not** prove that the program renders correctly on Intel Arc under Proton. Runtime testing on the target machine is still required.

## Repository layout

| Path | Purpose |
|---|---|
| `apps/victory` | Command-line executable |
| `crates/victory-gpu` | D3D12 objects, command lists, GPU memory and host pipelines |
| `crates/victory-shader` | Xbox shader translation |
| `crates/victory-hle` | Console API implementations |
| `crates/victory-loader` | Executable loading and startup |
| `crates/victory-core` | Configuration, logging and shared policy |
| `docs/architecture.md` | Architecture overview |
| `.github/workflows/ci.yml` | Automated checks |

## Updating your local clone

Pull changes from the fork's `main` branch:

```bash
git pull --ff-only
```

Then rebuild using the command in [Build on Arch Linux](#build-on-arch-linux).

## Credits and license

This fork builds on [Marvisak/VIctory](https://github.com/Marvisak/VIctory) and the upstream contributors and fork history recorded in Git. See [LICENSE](LICENSE) for the GPL-3.0-or-later license and the repository history for attribution of integrated work.
