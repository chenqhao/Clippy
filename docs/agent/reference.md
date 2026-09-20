# uclip agent brief — style, examples, testing, open questions, resources & first session

> Part of the uclip agent brief. Section numbers (§N) are the same in every file; the map is in the root `AGENTS.md` §8. Read this file with your file tools when the task touches this topic.

---

## 15. Code Style & Conventions

- **Error messages:** lowercase, no trailing period. Example: `error: peer "home-desktop" is not paired`.
- **Output prefixes:** `▸` (cyan) info, `✓` (green) success, `✗` (red) error, `⚠` (yellow) warning. Data goes to stdout; everything else goes to stderr (§10.2).
- **Naming:** `snake_case` for functions, variables, and modules; `PascalCase` for types and traits; `SCREAMING_SNAKE_CASE` for constants.
- **No `unwrap()` outside tests.** Use `?`, `.context("...")`, or an explicit `match`. `expect("...")` is allowed only for genuine invariants, with a message that states the invariant — and never on I/O, network, crypto, or user input.
- **`unsafe`:** `#![deny(unsafe_code)]` at the crate root. It is allowed only in `platform/` and `clipboard/sensitive.rs`, behind a local `#[allow(unsafe_code)]`, with a `// SAFETY:` comment on every block (Phase 10+).
- **Docs:** every public item has a `///` doc comment; every module has a top-level `//!` comment (`#![warn(missing_docs)]`).
- **Lints:** `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` in CI. Explain each lint the first time it fires.
- **Ownership hygiene:** every `.clone()` needs a reason. When the user writes one, ask: "Who owns this, and why do we need a second owner?" Prefer `&str`, `&[T]`, and `&Path` in parameters; return owned types.
- **Errors:** `thiserror` in library-like modules, `anyhow` at the edges (`main`, command handlers). Error *messages* are for humans; error *types* are for code.
- **Async hygiene:** no blocking calls inside `async fn`; no `MutexGuard` across `.await`; every spawned task has a cancellation path; every channel is bounded unless a comment justifies otherwise.
- **Logging:** `tracing` macros with structured fields; never log clipboard contents, keys, or pairing codes.
- **Tests:** names describe behavior (`remote_update_older_than_current_is_ignored`); one behavior per test; no test touches the real clipboard or the real network unless it is marked `#[ignore]` and documented as manual.
- **Commits:** small, imperative subject (`add Paths struct with UCLIP_HOME override`); each commit compiles and passes tests.
- **Dependencies:** each new one is justified in its commit message (§4.1).

---

## 16. Example Interactions

```
$ uclip init
  ▸ Creating config and data directories
  ✓ Device "work-laptop" initialized (id b6f0c7a2)
  ✓ Identity key generated
  ▸ Next: `uclip start`, then pair another device (`uclip pair listen` / `uclip pair connect`)

$ uclip start
  ▸ Starting uclipd...
  ✓ uclipd is running (pid 4821)

$ uclip status
  Daemon:      running (pid 4821, up 2h 14m, v0.3.0)
  Device:      work-laptop (b6f0c7a2)
  Sync:        enabled
  Peers:       home-desktop ✓ online (192.168.1.20)  |  phone ✗ offline
  Last sync:   12s ago ← home-desktop (text, 84 bytes)

$ uclip devices
  NAME            ID        STATE     ADDRESS
  home-desktop    3f2c9b1e  online    192.168.1.20:47001
  phone           91ad07c4  offline   —
  ▸ Discovered, unpaired (pairing mode): none
```

Pairing, in two terminals:

```
# on work-laptop
$ uclip pair listen
  ▸ Pairing mode on for 120s. Waiting for another device...
  ▸ "home-desktop" is connecting
  ▸ Confirmation code:  482 913
  ? Does the other device show the same code? [y/N] y
  ✓ Paired with "home-desktop"

# on home-desktop
$ uclip pair connect
  ▸ Devices in pairing mode:
      1) work-laptop  (192.168.1.12)
  ? Pick a device [1]: 1
  ▸ Confirmation code:  482 913
  ? Does the other device show the same code? [y/N] y
  ✓ Paired with "work-laptop"
```

Local clipboard helpers and explicit send:

```
$ echo "hello from the terminal" | uclip copy
$ uclip paste
hello from the terminal
$ uclip send "deploy key rotated"
  ✓ Sent to 1 device (home-desktop)
```

Diagnostics:

```
$ uclip doctor
  Checking work-laptop...
  ✓ config and data directories writable
  ✓ device identity present
  ✓ identity key permissions (0600)
  ✓ clipboard readable
  ✓ daemon reachable (v0.3.0)
  ✓ listening on port 51234, advertising _uclip._tcp
  ⚠ peer "phone" not seen in 3 days
  ✗ autostart not installed — run `uclip autostart enable`
  Result: 6/8 checks passed, 1 warning
```

A failure with an exit code:

```
$ uclip stop
  ✗ error: daemon is not running (start it with `uclip start`)
$ echo $?
3
```

---

## 17. Example Coaching Exchanges

These show the *style* to follow: questions first, the user does the work, and you confirm understanding.

**A — The user hits a borrow-checker error**

```
User: error[E0382]: borrow of moved value: `text` — I don't get it.
You:  Read the error top to bottom. Which line moved `text`, and which line used it
      afterwards? Say it in your own words.
User: I pass it to `send()` and then log it.
You:  Right. What does `send(text: String)` take — a copy, a reference, or ownership?
      List two ways you could still log it before I show you any code.
User: Clone it, or pass a reference?
You:  Both work. Which would you pick for a 10 MiB clip, and why? Try it, then run
      `rustc --explain E0382` and tell me what the explanation adds.
```

**B — Introducing a crate (protocol from §3.1), then the first increment**

```
User: How do I read the clipboard?
You:  Pick the tool first: `arboard` is the most widely used cross-platform option; the
      alternative, `clipboard-rs`, adds change watching and custom formats — we'll evaluate it
      in Phase 10. I'm adding `arboard` with `cargo add arboard`; here is what changed in
      `Cargo.toml`: (diff and explanation).
      Increment 1 of 3 — a tiny program that only *reads* text, in `examples/arboard_hello.rs`:
      (about 8 lines, with a `//` comment on the why of each non-obvious line)
      Why it's shaped this way: `Clipboard::new()` returns a `Result` because the OS can refuse
      access, and `get_text()` errors when the clipboard holds an image — so "empty" and "error"
      are different cases we'll model properly in Phase 2. I ran `cargo run --example
      arboard_hello`: (output). Next increment: writing text back. Ready?
```

**C — A recurring pattern: the user's turn**

```
You:  We've written one clap subcommand (`init`) together. `copy` has the same shape, so this one
      is yours: an optional `text` argument that calls `commands::copy::run(text)`. Look at how
      `Init` is declared in `cli.rs` and write it.
User: (writes the variant and the handler)
You:  Close. The handler is right, but you declared `text: String`, which makes the argument
      required. Corrected: `text: Option<String>` (code). Why: clap infers "optional" from
      `Option<T>`, so the type *is* the specification — the same reason we avoid sentinel values
      (§2.5 rule 2). Compare it with `Init` once more. Next up is `paste`, and that one is yours too.
```

**D — A design decision**

```
User: Should I use JSON or a binary format on the wire?
You:  JSON is human-readable and easy to debug with `nc` and Wireshark, but bulky for binary
      blobs. Formats like `postcard` or MessagePack are compact and fast but harder to
      eyeball. For a text-first MVP, which risk matters more to you? Decide, write a short
      ADR in `docs/decisions/`, and I'll review it.
```

---

## 18. Testing Strategy

| Type | What | How |
|---|---|---|
| Unit | Sync engine rules, clock ordering, suppression, config parsing, protocol (de)serialization, framing, SAS derivation, sensitive-content decisions | `#[cfg(test)] mod tests` in each module; fakes at every OS boundary |
| Property | Convergence, idempotence, no-echo; JSON round-trips; the frame decoder never panics on arbitrary bytes | `proptest` |
| Snapshot | Wire JSON for every message; `--help` text; `status` / `doctor` output | `insta` |
| Integration (CLI) | `init` idempotency, `copy` / `paste`, error messages and exit codes | `tests/` directory, `assert_cmd`, `predicates`, `tempfile`, a temporary `UCLIP_HOME` |
| Integration (async) | Two engines connected by `tokio::io::duplex`; daemon lifecycle (start / status / stop, stale socket) | `#[tokio::test]`, temporary `UCLIP_HOME`, `UCLIP_BACKEND=memory` |
| Security | Unpinned peer rejected; tampered, truncated, and oversized frames handled without panics; handshake timeout | Dedicated tests; `cargo-audit` in CI; stretch: `cargo-fuzz` on the frame decoder |
| Manual, real devices | The checklist below | Record results in `docs/platform-notes.md` |

**Manual cross-device checklist**

- [ ] Copy ASCII text on A, paste on B — every direction and every OS pair.
- [ ] Unicode, emoji, CJK, and multi-line text; Windows vs Unix line endings.
- [ ] Large text (about 5 MiB), and text above the size cap (must be skipped with a warning).
- [ ] Copy the same text on both devices; copy on both at nearly the same moment.
- [ ] Sleep / wake; Wi-Fi off / on; switching networks; VPN on / off.
- [ ] Restart the daemon on one side; kill it hard (`kill -9` / Task Manager); reboot.
- [ ] Unpair while connected; pair again.
- [ ] Copy from a password manager (must **not** sync).
- [ ] Firewall prompts accepted and declined; an isolated guest network (fallback to a manual peer).

---

## 19. Open Design Questions (each becomes an ADR when decided)

1. **Watcher:** polling only, or event-driven per OS? What CPU or latency threshold justifies the extra complexity?
2. **Ordering:** plain Lamport clock vs hybrid logical clock vs a persisted counter.
3. **Pairing:** SAS numeric comparison (default) vs SPAKE2 short code vs QR code.
4. **Transport security:** Noise (`snow`) vs TLS with pinned certificates vs QUIC (`quinn`) — and what each means for Android.
5. **Serialization:** JSON throughout, or a binary format once images arrive? Measure first.
6. **Applying incoming clips:** automatic (like Apple) or a notify/confirm mode, given the clipboard-injection risk?
7. **Topology:** full mesh vs forwarding through intermediate peers, if more than three devices are ever supported.
8. **Key storage:** `0600` file vs OS keychain (`keyring`) — and the Windows story.
9. **Windows autostart:** Run key vs Scheduled Task.
10. **Binaries:** two (`uclip` + `uclipd`) vs one with a hidden `daemon` subcommand.
11. **History:** none (default), an in-memory ring buffer, or on-disk (encrypted) — and the privacy cost of each.
12. **Off-LAN use:** rely on an overlay network with manual peers, or build a relay (out of scope for now).
13. **Android:** native Kotlin vs a Rust core through UniFFI; which sync directions are acceptable.

---

## 20. Learning Resources

Point the user to the exact chapter or page at the moment it becomes useful — not all at once. Chapter titles below are from the Rust Book; numbering can shift between editions.

| Phase | Rust Book chapters | Other reading |
|---|---|---|
| 0–1 | Getting Started; Common Programming Concepts; Understanding Ownership; Using Structs to Structure Related Data; Enums and Pattern Matching; Managing Growing Projects with Packages, Crates, and Modules; Error Handling | The Cargo Book (intro); Command Line Applications in Rust |
| 2 | Generic Types, Traits, and Lifetimes; Writing Automated Tests | Rust by Example (traits) |
| 3 | Common Collections; Functional Language Features: Iterators and Closures; Fearless Concurrency | `std::sync` and `std::thread` docs |
| 4 | Patterns and Matching; Iterators and Closures (again) | Serde docs on enum representations; the proptest book |
| 5–6 | Fundamentals of Asynchronous Programming; Smart Pointers | The Tokio tutorial (spawning, channels, `select!`, framing) |
| 8 | — | The Noise Protocol Framework spec (skim the handshake patterns); RustCrypto docs |
| 9–10 | Advanced Features (unsafe, macros) | The Rustonomicon (only the `unsafe` and FFI introductions) |
| 11 | More About Cargo and Crates.io | Cargo Book: workspaces; the Rust API Guidelines |

Links:

- The Rust Programming Language ("the Book"): https://doc.rust-lang.org/book/
- Rust by Example: https://doc.rust-lang.org/rust-by-example/
- Rustlings (exercises): https://github.com/rust-lang/rustlings
- The Cargo Book: https://doc.rust-lang.org/cargo/
- Standard library docs: https://doc.rust-lang.org/std/
- Compiler error index: https://doc.rust-lang.org/error_codes/
- Clippy lint index: https://rust-lang.github.io/rust-clippy/master/
- Command Line Applications in Rust: https://rust-cli.github.io/book/
- Tokio tutorial: https://tokio.rs/tokio/tutorial
- Comprehensive Rust (Google): https://google.github.io/comprehensive-rust/
- Rust API Guidelines: https://rust-lang.github.io/api-guidelines/
- Noise Protocol Framework: https://noiseprotocol.org/
- Crate documentation: https://docs.rs

---

## 21. First Session: Do This Now

1. **Do not write any project code yet.**
2. Ask the user one short batch of calibration questions: which OS(es) and devices they can test on (Windows PC, Mac, Android phone), which editor they use, how much time they have per week, whether iOS matters to them, and whether they learn best from reading docs, videos, or exercises.
3. Restate the scope honestly: the MVP is desktop text sync (Phases 0–9); Android is a stretch (Phase 12); iOS is manual-only. Mention the two facts most likely to surprise them — the daemon must be a per-user login item (§7.2), and many campus and guest Wi-Fi networks block device-to-device traffic (§7.5), so they should test on a home network or a phone hotspot.
4. Start **Phase 0** one step at a time. The user installs tools themselves (explain each command before they run it); after that, follow the coding protocol in §2.3, checking understanding before moving on.
5. End the session with the wrap-up from §2.7: two questions, an entry in `docs/learning-log.md`, and the next task.
