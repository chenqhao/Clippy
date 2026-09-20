# uclip agent brief — sync engine, discovery, pairing & security

> Part of the uclip agent brief. Section numbers (§N) are the same in every file; the map is in the root `AGENTS.md` §8. Read this file with your file tools when the task touches this topic.

---

## 12. Sync Engine Design

The core is a pure struct (§6.4). Design the concrete types together with the user; the tables below are the *behavior* to elicit through questions before you write any code.

### 12.1 Model

| Piece | Meaning |
|---|---|
| State | `device_id`; logical `clock`; `current` clip record (`update_id`, `origin`, `clock`, content hash, size); suppression map (hash → expiry); `paused`; limits from config |
| Events in | `LocalChange(content)`, `RemoteUpdate { from, update }`, `PeerConnected(peer)`, `Tick(now)`, `SetPaused(bool)` |
| Actions out | `Broadcast(update)`, `ApplyToClipboard(content)`, `SendTo(peer, update)`, `Log(level, message)` (never contents) |

### 12.2 Rules

**On a local change:**
1. If paused or sync disabled → ignore.
2. Hash the content. If it matches an unexpired suppression entry (we just wrote it) → **echo, ignore**. If it equals `current`'s hash → **duplicate, ignore**.
3. If it exceeds `max_item_bytes` or its type is not enabled → ignore and log a warning (size and type only).
4. If flagged concealed and `respect_concealed` → ignore (Phase 10).
5. Advance the clock, build the `ClipUpdate`, set `current`, emit `Broadcast`.

**On a remote update:**
1. Validate: size, type, protocol version, and that `origin` equals the **authenticated** peer the message arrived from. A peer must never be able to forge another device's `origin`.
2. Merge the clock (Lamport receive rule: take the max of local and remote).
3. Compare `(remote.clock, remote.origin)` with `(current.clock, current.origin)`. Newer wins; equal or older is ignored. (The `origin` tiebreak makes the order total, so every device picks the same winner.)
4. If adopted: set `current`, add the hash to the suppression map with a short expiry (about 2 s), emit `ApplyToClipboard`.
5. **Do not re-broadcast.** In a full mesh, the origin already sent it to everyone. (Forwarding for non-fully-connected topologies is a later ADR.)

### 12.3 Design problems to elicit (ask, don't tell)

- **Echo loops:** "Device B writes a remote clip to its clipboard. What does B's watcher do next?" → suppression by content hash with expiry. Follow-up: "What if the OS or clipboard library changes the text slightly (line endings, normalization) so the hash no longer matches?" Let the user test this on both OSes.
- **Ordering after restart or offline copies:** "Device B restarts and its counter goes back to zero. What happens to the next thing you copy on B?" The plain Lamport design has a real flaw here. Let the user discover it with a scenario test, then guide them toward a hybrid logical clock or a persisted counter, and have them record the choice in an ADR. (A wire change means bumping `protocol_version` — a free lesson in versioning.)
- **Initial sync on connect:** should a newly connected peer receive the current clip? Discuss what happens to a newer local clip on the other side, and how the ordering rule protects it.
- **Debounce and coalescing:** some apps update the clipboard many times per second (spreadsheets, terminal selections). What is the right debounce, and what is the cost of being too slow or too fast?
- **Latest-only delivery:** "Does a slow or offline peer need every clip you copied, or only the latest?" → a `watch`-style single slot per peer rather than a growing queue.

### 12.4 Connection management

- **Who dials whom?** Both sides discover each other, so both might dial. Rule: the device with the smaller `DeviceId` dials; the other only listens. (A lesson in `Ord` on newtypes.) Also handle a duplicate connection that slips through: keep one, send `Goodbye { Duplicate }` on the other.
- **Reconnect** with exponential backoff and jitter; reset the backoff after a stable connection.
- **Keepalive:** `Ping` about every 15 s; drop after about 45 s of silence.
- **Limits:** cap concurrent unauthenticated connections, handshake time, frame size, and connections per peer.

### 12.5 Properties to test (Phase 4, with `proptest`)

- **Convergence:** delivering the same set of updates in any order yields the same `current` everywhere.
- **Idempotence:** applying the same update twice changes nothing.
- **No echo:** an applied remote update never produces a `Broadcast`.
- **Monotonic clock:** the clock never goes backwards.
- **Size and type limits** are always enforced.

---

## 13. Discovery, Pairing & Security

### 13.1 Discovery (mDNS / DNS-SD)

- Service type `_uclip._tcp.local.`; listen on an **ephemeral** port (bind to port 0, read back `local_addr()`, advertise it). `network.listen_port` can pin it for firewall rules.
- Steady-state TXT records: `v` (protocol version) and `fp` (a short fingerprint — the first bytes of a hash of the device's public key). Paired devices recognize each other by `fp`; strangers on the LAN learn nothing useful.
- In **pairing mode only**, also advertise `pair=1` and the device `name`, so the other device can show a pick-list.
- Advertise all usable addresses. Handle multiple interfaces, IPv4 and IPv6, and address changes (a peer's IP can change; mDNS updates should trigger a reconnect).
- `manual_peers` and `uclip peer add` are the fallback when multicast is blocked (§7.5).

### 13.2 Pairing flow (recommended default: Noise XX + numeric comparison)

Present the alternatives (SPAKE2 short-code entry via the `spake2` crate; QR code carrying a high-entropy secret; TLS with pinned self-signed certificates), give the tradeoffs, and let the user decide via an ADR. The default below teaches the most with the least crypto surface:

1. **A:** `uclip pair listen` → the daemon enters pairing mode (time-limited, one session at a time) and advertises `pair=1`.
2. **B:** `uclip pair connect` → lists devices in pairing mode; the user picks one; B dials A.
3. Both run a **Noise `XX`** handshake with their long-term static keys. Neither trusts the other yet, but each learns the other's static public key:

```
XX:   → e
      ← e, ee, s, es
      → s, se
```

4. After the handshake, both compute a **short authentication string (SAS)** — a 6-digit code derived from the final handshake hash — and show it on both CLIs. (Verify how to read the handshake hash in the `snow` docs. The code must be derived *after* the final handshake message.)
5. The users compare the codes and answer y/N on each device. Both sides send `PairConfirm { accepted }` inside the encrypted channel. Only if **both** accept, each side stores the other's id, name, and public key in `peers.toml`.
6. Pairing mode ends. Every later connection runs Noise `XX` again, and the daemon **rejects any remote static key that is not pinned** in `peers.toml`. Unknown keys, mismatches, and bad handshakes are logged (without secrets) and dropped.

Have the user draw the handshake on paper and explain what each token (`e`, `s`, `ee`, `es`, `se`) means before writing code.

### 13.3 Threat model (the user writes `docs/threat-model.md` **before** any crypto code)

Prompt them to cover:

- **Assets:** clipboard contents (passwords, tokens, personal data); the device's private key; the list of trusted peers.
- **Adversaries:** a passive LAN eavesdropper; an active LAN attacker (man-in-the-middle, rogue mDNS responder, spoofed peer, connection flooding); malware running as the same user (it can already read the clipboard and can talk to the IPC socket); a lost or stolen paired device; a *malicious paired peer*.
- **Clipboard injection:** a paired peer can put arbitrary text — including multi-line shell commands — on your clipboard, and you may paste it into a terminal without looking. Discuss mitigations (unpair, a "notify/confirm before applying" mode as a later feature).
- **Explicitly out of scope:** a compromised paired device, malware with the user's privileges, traffic-analysis metadata (a LAN observer can see that two hosts talk).
- For each threat: mitigation, or an honest "not mitigated".

### 13.4 Privacy rules

- Never log contents. Implement `Debug` manually for types that hold clipboard data or secrets so they print as redacted — a natural first *manual trait impl*.
- Honor **concealed / sensitive** hints set by password managers (Phase 10): for example the macOS `org.nspasteboard.ConcealedType` marker and the Windows clipboard formats `ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory`, and `CanUploadToCloudClipboard` (verify the current conventions). Default `respect_concealed = true`.
- Enforce size limits, keep no history by default, and keep clipboard data in memory only (no writing contents to disk).

### 13.5 Crypto ground rules

- No custom primitives or protocols. Use `snow`. Handle every handshake and decrypt error explicitly — no `unwrap`.
- Validate lengths **before** allocating buffers from network-supplied sizes.
- Time-limit handshakes; cap unauthenticated connections; verify frame length limits *before* decrypting.
- Wipe secrets with `zeroize`; compare keys and codes with constant-time comparison (`subtle`).
- Say honestly, in the README and in conversation, that this is an unaudited learning project and should not protect high-value secrets.
