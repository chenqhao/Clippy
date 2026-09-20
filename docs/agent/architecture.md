# uclip agent brief — overview, structure, architecture, platforms, config & protocols

> Part of the uclip agent brief. Section numbers (§N) are the same in every file; the map is in the root `AGENTS.md` §8. Read this file with your file tools when the task touches this topic.

---

## 1. Project Overview

**uclip** (working name) is a cross-platform "universal clipboard": copy on one device, paste on another. It is modeled on Apple's Universal Clipboard but works across Windows, macOS, and Android. It is built in Rust and consists of:

- **`uclipd`** — a background daemon on every device. It watches the local clipboard, discovers paired devices on the local network, and syncs clipboard changes over an authenticated, encrypted connection.
- **`uclip`** — a CLI that initializes a device (`uclip init`), pairs it with other devices, and controls/inspects the daemon (`start`, `stop`, `status`, `devices`, `doctor`, ...).
- **Android companion app** (post-MVP) — a Kotlin app speaking the same wire protocol.

**Target platforms:** Windows 10/11 and macOS (full peers); Linux/X11 (stretch); Android 10+ (companion app, with asymmetric limits — see §7); iOS (manual share only — see §7).
**Language:** Rust. This is also a learning project, and **the user is completely new to Rust** — treat every implementation decision as a teaching opportunity.
**MVP (Phases 0–9):** text sync between two desktop machines on the same LAN, with device pairing, encrypted transport, a controllable daemon, autostart at login, and a `doctor` command.
**Non-goals (for now):** cloud relay / off-LAN sync, clipboard history UI, file transfer, iOS background sync, GUI or tray app, multi-user support.

### 1.1 Why Rust (decision made)

- A long-running background process benefits from having no garbage collector and from small, predictable memory use.
- Enums plus exhaustive `match` are ideal for modeling protocol messages and state machines.
- One toolchain (`cargo`) builds for all three desktop OSes and produces single native binaries.
- A Rust core can later be reused on Android through `cargo-ndk` + UniFFI.
- Honest tradeoff: Go would reach a working MVP sooner. Rust was chosen because learning Rust is the point of the project. If the user is ever tempted to switch languages to "go faster", remind them of that.

---

## 5. Project Structure

Do **not** create all of this up front. Each file appears in the phase that needs it (see §14). Phases 0–10 use one Cargo *package* containing a library and two binaries; Phase 11 refactors it into a workspace.

### 5.1 Single-package layout (Phases 0–10)

```
uclip/
├── Cargo.toml
├── Cargo.lock                    # committed (this is an application, not a library)
├── rust-toolchain.toml           # pins toolchain + components (rustfmt, clippy)
├── README.md
├── AGENTS.md                     # core agent rules (short: Antigravity caps rules files at 12,000 chars)
├── LICENSE-MIT, LICENSE-APACHE   # Phase 11
├── deny.toml                     # cargo-deny policy (Phase 8/11)
├── justfile                      # optional: `just check` = fmt + clippy + test
├── .github/workflows/ci.yml      # fmt, clippy, test on ubuntu / windows / macos
├── docs/
│   ├── agent/                    # the detailed agent brief, read on demand (see AGENTS.md §8)
│   ├── learning-log.md           # the user's own notes, one entry per session
│   ├── toolbox.md                # the user's cheat sheet of tools and crates
│   ├── decisions/                # ADRs: 0001-language-rust.md, 0002-..., written by the user
│   ├── threat-model.md           # written BEFORE any crypto code (Phase 8)
│   ├── protocol.md               # wire + IPC protocol spec, kept in sync with `proto/`
│   ├── platform-notes.md         # per-OS findings: firewalls, permissions, quirks
│   └── android-plan.md           # Phase 12
├── examples/                     # tiny scratch programs for learning each crate
├── packaging/                    # templates: macos/*.plist, linux/*.service (see §11)
├── src/
│   ├── main.rs                   # `uclip` CLI entry: parse args, dispatch, map errors to exit codes
│   ├── bin/
│   │   └── uclipd.rs             # `uclipd` daemon entry (Phase 5): build runtime, call `daemon::run`
│   ├── lib.rs                    # library root: declares modules; shared by both binaries
│   ├── cli.rs                    # clap definitions (all commands, subcommands, flags)
│   ├── commands/                 # one file per CLI command — thin: parse → call library → print
│   │   ├── mod.rs
│   │   ├── init.rs   config.rs   copy.rs    paste.rs    watch.rs
│   │   ├── start.rs  stop.rs     status.rs  logs.rs
│   │   ├── pair.rs   devices.rs  unpair.rs  peer.rs     send.rs   pause.rs
│   │   └── autostart.rs   doctor.rs   completions.rs
│   ├── config/
│   │   ├── mod.rs
│   │   ├── paths.rs              # `Paths`: config/data dirs, socket path, `UCLIP_HOME` override
│   │   ├── settings.rs           # `config.toml` schema (+ defaults)
│   │   ├── device.rs             # `device.toml`: DeviceId + display name
│   │   └── peers.rs              # `peers.toml`: paired devices (load / save / add / remove)
│   ├── clipboard/
│   │   ├── mod.rs                # `ClipboardBackend` trait, backend factory (`UCLIP_BACKEND`)
│   │   ├── system.rs             # real clipboard via `arboard`
│   │   ├── memory.rs             # in-memory fake (unit tests)
│   │   ├── file.rs               # file-backed fake (run two daemons on one machine)
│   │   ├── error.rs              # `ClipboardError` (thiserror)
│   │   ├── watcher.rs            # `ClipboardWatcher` trait + polling implementation
│   │   └── sensitive.rs          # concealed/sensitive-content detection, cfg-gated per OS (Phase 10)
│   ├── proto/
│   │   ├── mod.rs
│   │   ├── message.rs            # wire `Message` enum, `ClipContent`
│   │   ├── codec.rs              # framing (length-delimited) + JSON (de)serialization
│   │   └── version.rs            # PROTOCOL_VERSION + compatibility rules
│   ├── sync/
│   │   ├── mod.rs
│   │   ├── engine.rs             # PURE sync logic (no I/O): events in → `Action`s out
│   │   ├── clock.rs              # Lamport clock + ordering
│   │   ├── suppress.rs           # echo suppression / dedupe (hash + expiry)
│   │   └── actor.rs              # async wrapper: owns the engine, talks over channels
│   ├── net/
│   │   ├── mod.rs
│   │   ├── discovery.rs          # `Discovery` trait + mDNS implementation
│   │   ├── listener.rs           # accept loop
│   │   ├── connector.rs          # dial + reconnect with backoff
│   │   ├── peer.rs               # per-connection task; `PeerHandle`
│   │   ├── manager.rs            # peer registry; duplicate-connection rule
│   │   ├── secure.rs             # Noise handshake + encrypted stream (Phase 8)
│   │   └── pairing.rs            # pairing mode, SAS derivation, confirmation
│   ├── ipc/
│   │   ├── mod.rs
│   │   ├── protocol.rs           # `Request` / `Response` enums
│   │   ├── server.rs             # runs inside the daemon
│   │   └── client.rs             # used by the CLI
│   ├── daemon/
│   │   ├── mod.rs                # `run()`: startup sequence, wiring, shutdown
│   │   ├── lifecycle.rs          # single-instance check, signals, stale-socket cleanup
│   │   └── state.rs              # status snapshot published over a `watch` channel
│   ├── platform/                 # everything OS-specific lives here (cfg-gated)
│   │   ├── mod.rs
│   │   ├── detach.rs             # spawn `uclipd` detached (Unix vs Windows flags)
│   │   └── autostart/            # mod.rs, macos.rs, linux.rs, windows.rs
│   ├── identity.rs               # device keypair generate / load / store (Phase 8)
│   ├── doctor.rs                 # `Check` trait + individual checks
│   ├── error.rs                  # shared error types where a module boundary needs one
│   └── output.rs                 # colored/prefixed CLI output helpers (▸ ✓ ✗ ⚠), `--json`
└── tests/
    ├── common/mod.rs             # helpers: temp `UCLIP_HOME`, run the CLI, wait for the daemon
    ├── cli_init.rs
    ├── cli_copy_paste.rs
    ├── daemon_lifecycle.rs
    └── two_peers_sync.rs
```

### 5.2 Workspace layout (Phase 11)

```
uclip/
├── Cargo.toml                    # [workspace] + [workspace.dependencies]
└── crates/
    ├── uclip-proto/              # wire + IPC types, codec, versioning (no I/O)
    ├── uclip-core/               # sync engine, clock, suppression (no I/O, no async)
    ├── uclip-clipboard/          # ClipboardBackend / Watcher traits + per-OS implementations
    ├── uclip-net/                # discovery, pairing, Noise transport
    ├── uclip-ipc/                # local-socket client and server
    ├── uclipd/                   # daemon binary: wires everything together
    ├── uclip/                    # CLI binary
    └── uclip-ffi/                # Phase 12: UniFFI bindings for Android (cdylib)
```

Dependency direction: `uclip-proto` at the bottom; `core`, `clipboard`, `net`, `ipc` depend on it; the two binaries and `ffi` sit on top.

### 5.3 When to split into crates

Split only when the user can state a **concrete benefit**: compile-time isolation, enforcing "no I/O / no async in core" (the crate simply doesn't depend on `tokio`), reuse by the FFI crate, or independent testing. "It looks tidier" is not enough. Have the user write the ADR before the refactor.

---

## 6. Architecture

### 6.1 Components

| Component | Runs as | Responsibility |
|---|---|---|
| Clipboard backend | Trait with `system`, `memory`, `file` implementations | Read and write the local clipboard |
| Clipboard watcher | Dedicated OS thread | Detect local changes, emit `LocalChange` events |
| Sync engine | One async task (actor) around a pure struct | Owns sync state; decides what to broadcast, apply, or ignore |
| Peer manager | One async task | Tracks peers and connections; routes messages; enforces the duplicate-connection rule |
| Peer connection | One task per connected peer | Framing, encryption, read/write loops, keepalive |
| Discovery | One task | mDNS register + browse → `PeerSeen` / `PeerGone` events |
| IPC server | One task + one per CLI connection | Serves `Request`s from the CLI |
| Supervisor | `main` of `uclipd` | Wires channels, owns the `CancellationToken`, handles signals, shuts down gracefully |

### 6.2 Data flow

```
 OS clipboard ◄──────── ClipboardBackend (trait) ◄──────────────┐ apply remote clip
      │                                                          │
      ▼ (poll / events)                                          │
 ┌───────────┐  LocalChange   ┌─────────────┐   Broadcast   ┌──────────────┐   TCP + Noise
 │  Watcher  │ ─────────────► │ Sync engine │ ────────────► │ Peer manager │ ◄────────────► other devices
 │ OS thread │                │   (actor)   │ ◄──────────── │ + conn tasks │
 └───────────┘                └──────▲──────┘  RemoteUpdate └──────▲───────┘
                                     │ Request / Response         │ PeerSeen / PeerGone
                              ┌──────┴──────┐              ┌──────┴──────┐
     uclip CLI ◄── socket ───►│ IPC server  │              │  Discovery  │ ◄── mDNS (multicast UDP)
                              └─────────────┘              └─────────────┘
```

### 6.3 Concurrency model (introduced gradually — Phase 3 for threads, Phase 5 for tasks)

| Channel | Type | Bounded? | Why |
|---|---|---|---|
| Watcher → engine | `tokio::sync::mpsc`, sent with `blocking_send` from the OS thread | Yes (e.g. 32) | Crosses the sync→async boundary; backpressure |
| Peer connection → engine | `mpsc` | Yes | Backpressure from a chatty or hostile peer |
| Engine → peer manager | `mpsc` | Yes | Same |
| Engine → per-peer outbound | `tokio::sync::watch` (latest value) | Latest-only | Only the *latest* clip matters; a slow peer must not build a backlog |
| Engine → clipboard writer | `mpsc` + `spawn_blocking` | Yes | OS clipboard calls block |
| Status snapshot | `watch` | Latest-only | IPC `status` reads current state without asking the engine |
| IPC request → reply | `oneshot` | n/a | One reply per request |
| Shutdown | `CancellationToken` | n/a | Cooperative cancellation of every task |

### 6.4 Design principles (teach these; make the user articulate the "why")

1. **Sans-IO core.** `sync::engine::Engine` is a plain struct with methods like `on_local_change(&mut self, ...) -> Vec<Action>` and `on_remote_update(&mut self, ...) -> Vec<Action>`. It does no I/O, no async, and never reads the clock or randomness itself (they are passed in). That makes it trivial to unit-test and property-test. The async actor is a thin shell around it.
2. **Traits at OS boundaries.** `ClipboardBackend`, `ClipboardWatcher`, `Discovery`: real implementations plus fakes, so tests never need real hardware.
3. **One owner per piece of state** (actor model). Prefer channels over `Arc<Mutex<_>>`.
4. **Bound everything:** channels, frame sizes, clip sizes, connection counts, handshake time.
5. **Never log clipboard contents.**
6. **Fail closed.** An unknown or mismatching peer is rejected, never "tolerated".

---

## 7. Platform Reality (raise these early; be honest about them)

### 7.1 Support tiers

| Platform | Role | Status | Hard limits to be upfront about |
|---|---|---|---|
| Windows 10/11 | Full peer (daemon + CLI) | MVP | A Windows *service* runs in an isolated session with no access to the user's clipboard, so the daemon must be a **per-user login item**. Firewall prompt on first listen. |
| macOS (current and previous major) | Full peer | MVP | No change-notification API (poll `NSPasteboard.changeCount`). Pasteboard-privacy and Local Network permission behavior is evolving — see §7.3 and §7.5. |
| Linux (X11) | Full peer | Stretch | Wayland restricts background clipboard access (compositor-dependent). |
| Android 10+ | Companion app | Post-MVP | Ordinary apps cannot read the clipboard in the background. Sync is asymmetric — see §7.4. |
| iOS | Manual only | Out of scope unless the user asks | No persistent background process; programmatic pasteboard reads trigger paste prompts. Best case is a Share extension or Shortcut. Say this plainly *before* the user invests any time. |

### 7.2 What "daemon" means here

`uclipd` is a **per-user background process started at login**, not a system service:

- Clipboard access needs the user's interactive session. A Windows Service runs in session 0 and cannot use the user's clipboard; a macOS LaunchDaemon (root, outside the GUI session) has the same problem.
- Therefore: macOS **LaunchAgent**, Windows **Run key or logon-triggered scheduled task**, Linux **`systemd --user`** unit (see §11.6).

### 7.3 Clipboard change detection per OS

| OS | Cheap change signal | Notes |
|---|---|---|
| Windows | `GetClipboardSequenceNumber()` counter (cheap to poll) or `AddClipboardFormatListener` (event-driven; needs a hidden message-only window) | Reading contents opens the clipboard, which fails if another app holds it → retry with a short backoff |
| macOS | `NSPasteboard.changeCount` (poll; no event API) | Apple has been previewing a pasteboard-privacy alert for *programmatic reads*: new `detect` methods that inspect the available types without reading data, and an `accessBehavior` property. As of the latest information available when this file was written (spring 2026) it was still an opt-in developer preview, but it may become the default in a newer macOS release. **Verify current behavior on the user's macOS version before the Mac side of Phase 3 and again in Phase 10.** Design implication: poll the change counter (no content read) and read content only when it changes. |
| Linux X11 | XFixes selection events, or polling | Optional |
| Linux Wayland | `wlr-data-control` or a successor protocol, where the compositor supports it | Not available in every compositor — verify |

The watcher trait must be designed as "tell me when something changed", so a polling implementation (Phase 3) and event-driven ones (Phase 10) are interchangeable.

### 7.4 Android and iOS

- **Android 10+:** only the default keyboard (IME) or the app with input focus can read the clipboard. Consequences (verify against the target Android version):
  - **Desktop → phone can be automatic:** a foreground service keeps a connection alive and *writes* the clipboard.
  - **Phone → desktop must be user-initiated:** Share sheet target, Quick Settings tile, or a notification action that briefly brings up a focused activity to read the clipboard. Accessibility-service and `adb`-granted-permission tricks exist; discuss their tradeoffs honestly before choosing one.
- **iOS:** out of scope for the MVP. If revisited, it is manual send via Share extension or Shortcuts. There is no background sync.

### 7.5 Network realities (put findings in `docs/platform-notes.md`)

- Many campus, guest, and public Wi-Fi networks enable **client isolation** or block multicast, so mDNS fails and devices cannot reach each other. Test on a home network or phone hotspot first, and provide the manual-peer fallback (`uclip peer add`).
- **Windows Firewall:** the first listen triggers a prompt; the user must allow *Private* networks. A network classified as *Public* blocks incoming connections by default.
- **macOS firewall:** may prompt for incoming connections for unsigned binaries. **Local Network privacy** (introduced in recent macOS versions): verify how it applies to unsigned command-line binaries launched from Terminal versus by launchd, and document what you find.
- VPNs and multiple network interfaces can make the daemon advertise or dial the wrong address. Advertise all usable addresses; try them in order.

---

## 8. Configuration & On-Disk Data

### 8.1 Locations

| What | Where | Why |
|---|---|---|
| `config.toml` (user-editable settings) | `ProjectDirs::config_dir()` | Conventional home for settings |
| `device.toml`, `identity.key`, `peers.toml`, `logs/`, `run/` | `ProjectDirs::data_local_dir()` | **Device-specific** state and secrets must not roam. On Windows, `config_dir()` is the *Roaming* profile, which can copy an identity to another machine |
| Everything, when `UCLIP_HOME` is set | `$UCLIP_HOME/{config,data}` | Lets several instances run on one machine, and isolates every test |

Use `ProjectDirs::from("dev", "uclip", "uclip")` (verify the qualifier/organization/application arguments on docs.rs). `uclip config path` prints the resolved locations. `ProjectDirs::state_dir()` exists only on Linux and returns an `Option` — a good lesson in why the type is `Option<PathBuf>`.

### 8.2 `config.toml`

Every field has a default; a missing file means "all defaults" (`#[serde(default)]` + `impl Default`).

```toml
[network]
listen_port = 0                # 0 = pick a free port automatically (advertised via mDNS)
discovery = true               # advertise and browse via mDNS
manual_peers = []              # fallback when mDNS is blocked, e.g. ["192.168.1.20:47000"]

[sync]
enabled = true
max_item_bytes = 10485760      # 10 MiB; larger clips are skipped with a warning
content_types = ["text"]       # later: "html", "png"
debounce_ms = 150

[privacy]
respect_concealed = true       # never sync items flagged concealed/sensitive by password managers
log_contents = false           # keep false; only ever flip for local debugging

[watcher]
mode = "auto"                  # "auto" | "poll" | "events"
poll_interval_ms = 300

[logging]
level = "info"                 # error | warn | info | debug | trace (RUST_LOG overrides)
file = true
```

### 8.3 `device.toml` (generated by `uclip init`, never hand-edited)

```toml
id = "b6f0c7a2-5c1e-4c53-9d7a-2f1f0f6d3e11"   # UUID v4; identifies this device
name = "work-laptop"                          # display name shown to peers (default: hostname)
created_at = 1789800000                       # unix seconds
```

### 8.4 `peers.toml` (managed by `uclip pair` / `unpair`)

```toml
[[peers]]
id = "3f2c9b1e-8a44-4e0a-b1d2-0c9e5a77f001"
name = "home-desktop"
public_key = "base64-encoded-x25519-public-key"
paired_at = 1789800500
paused = false
last_addr = "192.168.1.20:47001"    # hint only; never trusted for authentication
```

### 8.5 Other files and environment variables

- `identity.key` — the device's long-term private key (Phase 8). Unix permissions `0600`; on Windows rely on the per-user profile ACLs and document the limitation; evaluate OS key storage (`keyring`).
- `run/uclipd.sock` (Unix) or the named pipe `\\.\pipe\uclip-<hash of home dir>` (Windows) — the IPC endpoint. Unix socket paths have a short length limit (about 104 bytes on macOS, 108 on Linux): keep the path short and test with a long `UCLIP_HOME`.
- `logs/uclipd.log.<date>` — rotated daily.

| Variable | Meaning |
|---|---|
| `UCLIP_HOME` | Override every config/data location (multiple instances; test isolation) |
| `UCLIP_BACKEND` | Clipboard backend: `system` (default), `memory`, or `file:<path>` — for dev and tests |
| `RUST_LOG` / `UCLIP_LOG` | Log filter (e.g. `uclip=debug`) |
| `NO_COLOR` | Disable colored output |

---

## 9. Wire Protocol & IPC Protocol

`docs/protocol.md` is the source of truth. The user writes and maintains it; you review it against the code.

### 9.1 Layers

```
Message (Rust enum)
  → serde_json bytes
  → inner frame: [u32 big-endian length][payload]          application framing
  → split into ≤ 65,519-byte plaintext chunks              Noise limit: 65,535 − 16-byte tag
  → Noise-encrypt each chunk
  → outer frame: [u32 big-endian length][ciphertext]       `LengthDelimitedCodec`, with a max frame length
  → TCP
```

Phase 6 (plaintext, loopback-only, dev-only) skips chunking and encryption: outer frames carry the serialized message directly. Ask the user how a 5 MiB message can travel through a cipher with a 65,535-byte per-message limit — let them discover the chunking design before showing any code.

### 9.2 Application messages

| Message | Direction | Fields | Notes |
|---|---|---|---|
| `Hello` | Both; first message after the handshake | `protocol_version: u16`, `device_id`, `device_name`, `capabilities: Vec<Capability>` | Verify `device_id` matches the pinned key's owner in `peers.toml`; disconnect on mismatch |
| `ClipUpdate` | Either | `update_id: Uuid`, `origin: DeviceId`, `lamport: u64`, `created_at_ms: u64`, `content: ClipContent` | The only message that carries clipboard data |
| `Ping` / `Pong` | Either | `nonce: u64` | Keepalive about every 15 s; drop the connection after about 45 s of silence |
| `Goodbye` | Either | `reason: GoodbyeReason` (`Shutdown`, `Unpaired`, `ProtocolError`, `Duplicate`) | Sent before closing when possible |
| `PairConfirm` | Pairing sessions only | `accepted: bool` | Rejected outside a pairing session (§13.2) |

`ClipContent` is an enum: `Text { text }` (Phase 4), then `Html { html, text_fallback }` and `Png { bytes }` (Phase 10). Mark it `#[non_exhaustive]` and discuss why.

### 9.3 Versioning rules

- `protocol_version: u16` is sent in `Hello`. A change that old peers cannot parse bumps it.
- Decide, via an ADR, the policy for **unknown message types** (ignore vs close the connection) and for unknown fields.
- Snapshot-test the JSON of every message (`insta`) so accidental wire changes show up in review.

### 9.4 IPC (CLI ↔ daemon)

Same framing (`LengthDelimitedCodec`) and JSON encoding, over a local socket / named pipe.

- **Requests:** `Status`, `Stop`, `Ping`, `Peers`, `Send { text }`, `Pause`, `Resume`, `PairListen { timeout_secs }`, `PairConnect { target }`, `PairConfirm { accepted }`, `Unpair { peer }`, `AddManualPeer { addr }`.
- **Responses:** `Ok`, `Status(StatusInfo)`, `Peers(Vec<PeerInfo>)`, `Error { code, message }`.
- **Streaming events** for the pairing commands: the server keeps the connection open and pushes `PairingEvent::ShowCode { code }`, `Paired { peer }`, `Failed { reason }`. This is the user's first taste of a stream of responses over one connection.
- **Security:** any process running as the same user can talk to this socket. Restrict permissions (`0600` on Unix; verify the default ACLs for named pipes on Windows), and never put clipboard contents in a `Status` response.
