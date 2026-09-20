# uclip agent brief — implementation phases

> Part of the uclip agent brief. Section numbers (§N) are the same in every file; the map is in the root `AGENTS.md` §8. Read this file with your file tools when the task touches this topic.

---

## 14. Implementation Phases

Build the project in this order. Each phase must be fully working and tested before moving on, and every phase ends with something the user can *see* working — that keeps motivation up. Each phase lists the Rust concepts to introduce (only when the code needs them), the tools and crates to teach via §3, concrete tasks, a definition of done, and checkpoint questions to ask before moving on.

Be honest about scale: for someone new to Rust, the MVP (Phases 0–9) is a multi-week effort, and Phase 12 (Android) is closer to a second project. Say so early, and never let the user feel behind. Coding in every phase follows the coding protocol in §2.3: small explained increments, with the user writing recurring patterns.

### Phase 0 — Toolchain, Cargo & Rust Basics

**Rust concepts introduced:** what Rust is (compiled, ownership instead of a GC), project anatomy (`Cargo.toml`, `src/main.rs`, `target/`, `Cargo.lock`), `fn main`, macros vs functions (`println!`), variables (`let`, `mut`, shadowing), scalar types, functions and expressions, `if` / `loop` / `for` / `while`, comments and doc comments, `String` vs `&str` (first look only), modules (first look).
**Tools to teach:** `rustup`, `cargo` basics, `rustfmt`, `clippy`, rust-analyzer, git + GitHub, GitHub Actions.

- [ ] Install Rust with rustup; run `rustc --version` and `cargo --version`; explain stable / beta / nightly.
- [ ] Set up VS Code + rust-analyzer; turn on inlay hints; use hover and go-to-definition.
- [ ] `cargo new uclip`; run it; read `Cargo.toml`; explain `target/` and why it is git-ignored; compare `cargo build` and `cargo build --release`.
- [ ] Add `rust-toolchain.toml` (boilerplate is fine); run `cargo fmt` and `cargo clippy` and read what they say.
- [ ] Alongside: the first Rust Book chapters (Getting Started; Common Programming Concepts) and/or the first Rustlings exercises.
- [ ] Create the GitHub repo; add CI (matrix: ubuntu, windows, macos; steps: fmt check, clippy with `-D warnings`, test). Boilerplate is fine, but the user must be able to explain every step.
- [ ] Create `docs/` with `learning-log.md`, `toolbox.md`, and `decisions/` (first ADR: "0001 — Language: Rust").

**Done when:** CI is green on all three OSes, and the user can explain `cargo build` vs `check` vs `run`.
**Checkpoint:** Why does `println!` end in `!`? What is in `Cargo.lock`, and why commit it for a binary? What does `-D warnings` do?

### Phase 1 — Domain Types, Config & `uclip init`

**Rust concepts introduced:** `struct`, `enum`, `impl` (methods vs associated functions), `derive` (`Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Default`), `Option`, `Result`, `?`, `anyhow`, ownership and moves (first real encounter), borrowing (`&`, `&mut`), `String` vs `&str`, `PathBuf` vs `&Path`, `std::fs`, `Display` / `FromStr` / `From` (first look), the newtype pattern, modules and `pub`, `lib.rs` vs `main.rs`, unit tests, doc comments.
**Tools & crates to teach:** `clap` (derive), `serde` + `toml`, `uuid`, `directories`, `gethostname`, `anyhow`, `owo-colors`, `assert_cmd` + `predicates` + `tempfile`, `bacon`, `cargo-expand`.

- [ ] `DeviceId(Uuid)` newtype with `Display` and `FromStr`; round-trip test.
- [ ] `Paths` struct (`UCLIP_HOME` override + `directories`) with methods returning `PathBuf`; tests using `tempfile`.
- [ ] `DeviceInfo` and `Settings` structs with serde; a missing `config.toml` yields defaults (`#[serde(default)]`).
- [ ] clap skeleton with every subcommand from §10 (unimplemented ones print "not implemented yet" and exit non-zero). You write the first subcommand; the rest follow the same pattern, so they are the user's turn (§2.3).
- [ ] `uclip init` (idempotent, with `--name` and `--force`) and `uclip config path|show`.
- [ ] Integration tests: `init` twice; `--force`; everything under a temporary `UCLIP_HOME`.

**Done when:** `uclip init` works on the user's OSes and CI is green.
**Checkpoint:** Why `&str` in parameters but `String` in struct fields? What does `?` desugar to? What does `#[derive(Default)]` generate (show it with `cargo expand`)?

### Phase 2 — Clipboard Abstraction (local only)

**Rust concepts introduced:** traits (definition, implementation, default methods), generics and trait bounds, `dyn Trait` vs generics (`Box<dyn ClipboardBackend>`), `thiserror` enums with `#[from]`, `Result<Option<T>, E>` semantics, `match` on error variants, `#[cfg(test)]` fakes, reading stdin, `IsTerminal`.
**Tools & crates to teach:** `arboard`, `thiserror`, `just` (optional).

- [ ] Design the `ClipboardBackend` trait *together* first: which operations? `&self` or `&mut self`? How does "the clipboard is empty" appear in the types? Present the options, let the user choose, then write it incrementally (§2.3).
- [ ] `ClipContent` enum (start with `Text(String)`; discuss why an enum now).
- [ ] `SystemClipboard` (arboard), `MemoryClipboard` (fake), `FileClipboard` (a file acting as the clipboard, for running two daemons on one machine later).
- [ ] `ClipboardError` with `thiserror`; map arboard's "content not available" to `Ok(None)`.
- [ ] `UCLIP_BACKEND` selects the backend through a factory returning `Box<dyn ClipboardBackend>`.
- [ ] `uclip copy` / `uclip paste` (stdin / stdout, exit codes).
- [ ] Unit tests use only the fake; manually test the real clipboard on each OS.

**Gotchas:** the clipboard being briefly occupied by another app (Windows); clipboard ownership on Linux; empty vs non-text content. Check `arboard`'s current error variants on docs.rs.
**Done when:** copy/paste round-trips on real clipboards, and automated tests never touch the real clipboard.
**Checkpoint:** Generics vs `dyn` — when each? Why `Result<Option<String>>` instead of returning an empty string?

### Phase 3 — Clipboard Watcher (threads & channels)

**Rust concepts introduced:** `std::thread::spawn` and `JoinHandle`, `move` closures, `std::sync::mpsc`, `Send` / `Sync`, `Arc`, `AtomicBool`, `Duration` / `Instant`, `loop` + `sleep`, hashing to a `[u8; 32]`, arrays vs `Vec<u8>`, `Drop`, events as enums.
**Tools & crates to teach:** `sha2`, `tracing` + `tracing-subscriber`, `RUST_LOG`, `dbg!`.

- [ ] Design a `ClipboardWatcher` trait together ("tell me when it changed" — blocking iterator? callback? channel?), then write it incrementally.
- [ ] `PollingWatcher`: interval from config, compares content hashes, debounces.
- [ ] `uclip watch` (hash + length; `--show` for contents).
- [ ] Clean shutdown: Ctrl-C sets a flag, the thread exits, the main thread joins it.
- [ ] Research (do not implement yet) how event-driven detection works per OS (§7.3). Verify the current macOS pasteboard-privacy behavior on the user's Mac and record it in `docs/platform-notes.md`.

**Done when:** each change is reported exactly once, and idle CPU use is negligible (measure in Activity Monitor / Task Manager and discuss the polling-interval tradeoff).
**Checkpoint:** Why does the closure need `move`? Why is `Arc<AtomicBool>` fine across threads while `Rc` is not? What happens when the receiving end of a channel is dropped?

### Phase 4 — Protocol Types & the Sync Engine Core (pure logic)

**Rust concepts introduced:** enums with data, serde enum representations (`tag`, `rename_all`), derived ordering (`PartialOrd` / `Ord` compare fields in declaration order), `impl Ord`, `HashMap` / `HashSet` / `BTreeMap`, iterators and closures (`map`, `filter`, `fold`, `collect`), state machines, passing time in as a parameter, `#[non_exhaustive]`, property-based testing.
**Tools & crates to teach:** `serde_json`, `proptest`, `insta` (optional), `cargo-nextest`.

- [ ] Wire types in `proto` (`Hello`, `ClipUpdate`, `Ping` / `Pong`, `Goodbye`, `ClipContent`); JSON round-trip tests; snapshot the JSON so protocol changes are deliberate.
- [ ] `LamportClock`, `UpdateId`, `SyncState`.
- [ ] `Engine` with `on_local_change`, `on_remote_update`, and `tick(now)` returning `Vec<Action>`.
- [ ] Echo suppression, duplicate detection, size limits, and the last-writer-wins tiebreak — each with tests (§12.2).
- [ ] Property tests for convergence, idempotence, and no-echo (§12.5).
- [ ] Use a scenario test to expose the restart/offline ordering flaw (§12.3) and decide the fix in an ADR.

**Done when:** every rule in §12.2 has at least one test, and the property tests pass over thousands of random cases.
**Checkpoint:** Why is the engine pure, and what does that buy us? What does `derive(Ord)` compare first? What type does `collect()` infer here, and why?

### Phase 5 — Async Foundations, Daemon Skeleton & IPC

**Rust concepts introduced:** `async` / `await`, lazy futures, the `tokio` runtime (`#[tokio::main]`), `spawn`, `JoinHandle` / `JoinSet`, `select!`, `mpsc` / `oneshot` / `watch` channels, `CancellationToken`, signals, bridging sync→async (`blocking_send`, `spawn_blocking`), `Send + 'static`, why not to hold a `MutexGuard` across `.await`, propagating errors out of tasks.
**Tools & crates to teach:** `tokio`, `tokio-util`, `futures`, `interprocess`, `tracing-appender`, `nc` / `socat`, the CodeLLDB debugger, `tokio-console` (optional).

- [ ] `src/bin/uclipd.rs`: `main` builds the runtime and calls `daemon::run()`.
- [ ] Supervisor: cancellation token, signal handling, graceful shutdown, tasks tracked in a `JoinSet`.
- [ ] IPC protocol (`Request` / `Response`), server, and client: `Status`, `Stop`, `Ping`.
- [ ] Single-instance check and stale-endpoint cleanup (§11.3).
- [ ] Bridge the Phase 3 watcher thread into the engine with `blocking_send`; the engine logs local changes (metadata only).
- [ ] `uclip start` / `stop` / `restart` / `status` / `logs`; detached spawn; wait-for-ready polling (§11.4).
- [ ] File logging with rotation; keep the guard alive until exit.

**Done when:** `start → status → stop` works repeatedly, `kill -9` followed by `start` recovers, and no zombie sockets remain.
**Checkpoint:** What does `.await` actually do? What breaks if you call `std::thread::sleep` inside an async fn? Why must spawned futures be `Send + 'static`?

### Phase 6 — Localhost Networking (plaintext, dev-only)

**Rust concepts introduced:** `TcpListener` / `TcpStream`, framing with `LengthDelimitedCodec` and `Framed`, `Stream` / `Sink` (`StreamExt::next`, `SinkExt::send`), per-connection tasks, the actor + handle pattern (`PeerHandle` wraps a `Sender`), timeouts, reconnect with exponential backoff and jitter, code generic over `AsyncRead + AsyncWrite`, `thiserror` for network errors.
**Tools & crates to teach:** `tokio-util` (codec), `bytes`, `nc`, `tokio::io::duplex`.

- [ ] Listener and connector; `Hello` exchange; keepalive.
- [ ] Peer manager: connection map, the "smaller `DeviceId` dials" rule, duplicate handling.
- [ ] Wire the engine to the peer manager: broadcast `ClipUpdate`; apply remote clips through the backend.
- [ ] Two daemons on one machine with different `UCLIP_HOME`s and `UCLIP_BACKEND=file:<path>` sync with each other. **Loopback only, plaintext, clearly marked dev-only** (bind to `127.0.0.1`, never `0.0.0.0`).
- [ ] In-process integration test: two engines connected through `tokio::io::duplex`.

**Done when:** A→B and B→A sync, disconnect/reconnect works, and the logs show no echo loops.
**Checkpoint:** What does `Framed` give you? How do you write code that works for both `TcpStream` and a `DuplexStream`? Bounded vs unbounded channels — which, and why?

### Phase 7 — LAN Discovery (mDNS) & Manual Peers

**Rust concepts introduced:** `SocketAddr` / `IpAddr` and `FromStr`, iterating over event streams, the `HashMap` entry API, bridging a third-party channel into tokio, `Arc<Mutex<_>>` vs channels (revisited), `impl Trait` in return position, `Option` combinators.
**Tools & crates to teach:** `mdns-sd`, `dns-sd` / `avahi-browse`, Wireshark (`mdns` filter), OS firewall dialogs.

- [ ] `Discovery` trait + `MdnsDiscovery`: advertise the ephemeral port with TXT fields (§13.1), browse, emit `PeerSeen` / `PeerGone`.
- [ ] `uclip devices` shows paired and discovered devices.
- [ ] `manual_peers` and `uclip peer add`.
- [ ] Test between two real machines. Record what breaks (client isolation, firewalls, VPNs) in `docs/platform-notes.md`.

**Done when:** two machines find each other without typing an IP on a friendly network, and the manual fallback works on a hostile one.
**Checkpoint:** What does mDNS do that ordinary DNS doesn't? Why an ephemeral port plus TXT records? What happens when a peer's IP address changes?

### Phase 8 — Identity, Pairing & Encryption

**Rust concepts introduced:** newtype wrappers for keys, manual `impl Debug` for redaction, `Drop` + `zeroize`, constant-time comparison, byte slices `&[u8]`, `TryFrom`, `Vec<u8>` vs `[u8; N]`, enum-based state machines for the handshake, `thiserror` for crypto errors, `#[cfg(unix)]` file permissions, `map_err` chains, timeouts, chunking with `chunks()`, `BytesMut`.
**Tools & crates to teach:** `snow`, `base64`, `zeroize`, `subtle`, `dialoguer`, `keyring` (evaluate), Wireshark, `cargo-audit`.

- [ ] Write `docs/threat-model.md` first (§13.3). **No crypto code before you have reviewed it.**
- [ ] ADRs: pairing approach; Noise pattern and primitives; key storage (file vs OS keychain).
- [ ] Generate and store the identity key (extend `init`, still idempotent); `0600` on Unix; document the Windows story.
- [ ] Secure stream: Noise `XX` handshake → transport mode; chunking + inner framing (§9.1); pinned-key check.
- [ ] Pairing mode and SAS confirmation (§13.2); `peers.toml`; `uclip pair listen` / `connect`; `uclip unpair`.
- [ ] Reject unknown peers; handshake timeout; limit unauthenticated connections.
- [ ] Prove it: capture traffic in Wireshark and confirm the clipboard text is not visible; write tests that a rogue or unpinned peer is rejected and that tampered or truncated frames are handled without panics.

**Done when:** only paired devices sync, an unpaired device cannot, and the tampering / replay / truncation tests pass.
**Checkpoint:** What does Noise `XX` guarantee, and what does it not? Why a SAS? What does `zeroize` protect against, and what does it not?

### Phase 9 — OS Integration: Autostart, Robustness & `doctor`

**Rust concepts introduced:** `#[cfg(target_os = "...")]`, platform modules, `include_str!` templates, `std::process::Command` (exit status, captured output), `std::env::current_exe`, `ExitCode`, `Box<dyn Trait>` with `Vec<Box<dyn Check>>` (dynamic dispatch), error context.
**Tools to teach:** `launchctl`, `systemctl --user`, `reg` / `schtasks`, Console / Event Viewer / `journalctl`.

- [ ] `uclip autostart enable|disable|status` per OS (templates from §11.6; write the Rust for the first OS incrementally, then the other two OS modules are recurring patterns — the user's turn, §2.3).
- [ ] Make `uclip start` robust: detach flags, ready-wait, clear errors.
- [ ] `doctor` with trait-based checks and `--json` (§10.3).
- [ ] Logging polish and `uclip logs`.
- [ ] Log-out / log-in and reboot tests: the daemon is running and syncing again with no manual step, on Windows and macOS.

**Done when:** after a reboot, both machines sync with no manual steps. **This completes the MVP.**
**Checkpoint:** What does `#[cfg(...)]` do at compile time versus `if cfg!(...)`? Static vs dynamic dispatch — why is `dyn Check` right for `doctor`?

### Phase 10 — Rich Content, Images, Privacy & OS APIs (`unsafe` intro)

**Rust concepts introduced:** enum evolution and `#[non_exhaustive]`, `Cow`, `bytes::Bytes`, size caps and streaming, `unsafe` and FFI basics, RAII wrappers, `// SAFETY:` comments, `#![deny(unsafe_code)]` with local `#[allow]`, target-specific dependencies, `cfg_attr`.
**Tools & crates to teach:** `image`, `clipboard-rs` (evaluate), `windows-sys` / `windows`, `objc2` + `objc2-app-kit`, `cargo-expand`.

- [ ] `ClipContent::Png`: raw RGBA from `arboard` → PNG → wire → PNG → RGBA; enforce size limits.
- [ ] Concealed/sensitive detection per OS (§13.4) with `respect_concealed`; unit-test the decision logic using fake format lists.
- [ ] Event-driven watchers where feasible (Windows sequence number or listener; macOS `changeCount` plus type detection without content reads); keep polling as the fallback; compare CPU use.
- [ ] Optional HTML / rich text.
- [ ] Update `docs/platform-notes.md` with the current macOS pasteboard-privacy and Local Network behavior.

**Done when:** image copy syncs, a password-manager copy does not, and every `unsafe` block is tiny, wrapped, and documented.
**Checkpoint:** What exactly does `unsafe` allow? What invariants does your `// SAFETY:` comment claim, and who upholds them?

### Phase 11 — Workspace Refactor, Quality & Releases

**Rust concepts introduced:** Cargo workspaces (`[workspace]`, `members`, `[workspace.dependencies]`, `[workspace.package]`), crate API design (`pub`, `pub(crate)`, re-exports), Cargo features, semantic versioning, doc tests, `cargo doc`, MSRV, cross-compilation targets.
**Tools to teach:** `cargo-audit`, `cargo-deny`, `cargo-llvm-cov` (optional), `cargo-flamegraph` / `tokio-console` (optional), `cross` / `cargo-zigbuild`, `cargo-dist` (verify), GitHub Releases, `clap_complete`.

- [ ] Write an ADR for each crate split (§5.3); refactor to the layout in §5.2, keeping tests green at every step.
- [ ] `uclip completions <shell>` via `clap_complete`; colored-output polish; error messages that suggest a fix.
- [ ] CI: OS matrix, caching, `cargo deny`, `cargo audit`.
- [ ] README (install, quick start, how it works, security notes, limitations), licenses (MIT / Apache-2.0), CHANGELOG.
- [ ] Release pipeline: binaries for Windows, macOS, and Linux, plus an install script. Research macOS Gatekeeper / notarization and Windows SmartScreen and note the implications, even if signing is not implemented.
- [ ] Profile the idle daemon (CPU, memory, wakeups) and fix the top offender.

**Done when:** a tagged release produces downloadable binaries, and a fresh machine can install and pair in under five minutes by following the README.
**Checkpoint:** When is a workspace worth it? What counts as a semver-breaking change in a library crate?

### Phase 12 — Android Companion (stretch; decide the approach first)

**Rust concepts introduced:** FFI boundary design, `cdylib` / `staticlib` crate types, `uniffi`, JNI basics, who owns the async runtime across FFI, Kotlin basics (the user is new to it), the Android lifecycle (foreground service, Quick Settings tile, share target), QR pairing.
**Tools to teach:** Android Studio, `adb`, `cargo-ndk`, `uniffi-bindgen`, `qrcode` (desktop side), CameraX / ML Kit (Android side).

- [ ] Research the current Android clipboard rules on the target version and write `docs/android-plan.md` (verify: background reads, focus rules, notifications).
- [ ] ADR: (a) native Kotlin implementing the protocol vs (b) a Rust core via UniFFI. (b) reuses the crypto and protocol code once; (a) avoids FFI complexity but duplicates protocol and crypto logic in Kotlin. Present both honestly.
- [ ] Sync directions: desktop → phone automatic (write); phone → desktop user-initiated (share sheet / tile / notification action).
- [ ] QR pairing: the desktop shows a QR code with its address, public-key fingerprint, and a one-time token; the phone scans it.

**Done when:** desktop → phone works automatically, phone → desktop works via share/tile, and the platform limits are documented in the README.
**Checkpoint:** Where do the async runtime and the long-lived connection live in your design, and why? What does the FFI boundary look like, and what crosses it?

**iOS:** out of scope. If the user asks: Shortcuts / Share extension for manual send only; there is no background sync.
