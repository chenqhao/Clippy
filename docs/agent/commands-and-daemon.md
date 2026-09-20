# uclip agent brief — CLI commands & daemon lifecycle

> Part of the uclip agent brief. Section numbers (§N) are the same in every file; the map is in the root `AGENTS.md` §8. Read this file with your file tools when the task touches this topic.

---

## 10. Command Reference

### 10.1 Commands

| Command | Behavior |
|---|---|
| `uclip init [--name <name>] [--force]` | Create the config/data directories, `device.toml` (UUID + name; default name = hostname), and a default `config.toml` if absent. From Phase 8, also generate the identity key. **Idempotent and additive:** re-running fills in whatever is missing without touching existing data. `--force` regenerates the identity, which unpairs every device — warn loudly and ask for confirmation. |
| `uclip config path` / `uclip config show` | Print resolved directories / the effective configuration (defaults merged with the file). |
| `uclip copy [text]` | Put `text` (or stdin when no argument and stdin is not a terminal) on the **local** clipboard — like `pbcopy`. |
| `uclip paste` | Print the local clipboard text to stdout — like `pbpaste`. Exit 1 if the clipboard is empty or holds non-text. |
| `uclip watch [--show]` | Dev command: print local clipboard change events as hash + length. `--show` prints the contents (local debugging only). |
| `uclip start [--foreground]` | Start `uclipd` detached (or attached with `--foreground` for debugging). No-op if it is already running. Waits until the daemon answers on IPC, then reports. |
| `uclip stop` / `uclip restart` | Ask the daemon to shut down gracefully over IPC / stop then start. |
| `uclip status [--json]` | Daemon state: running or not, pid, uptime, version, connected peers, last sync (time, direction, size — **never content**), paused or not. |
| `uclip pair listen [--timeout <secs>]` | Put this device in pairing mode (default 120 s). When another device connects, show the confirmation code and ask for y/N. |
| `uclip pair connect [<name-or-addr>]` | Connect to a device that is in pairing mode (pick from a discovered list if no argument), show the code, ask y/N. |
| `uclip devices [--json]` (alias `peers`) | Paired devices (name, short id, online/offline, address, last seen) plus discovered-but-unpaired devices that are in pairing mode. |
| `uclip unpair <name-or-id>` | Remove trust: disconnect, delete from `peers.toml`. |
| `uclip peer add <host:port>` | Add a manual address hint for an already-paired peer when mDNS is blocked. |
| `uclip send [text]` | Explicitly push text (or stdin) to all online peers and set the local clipboard. Useful for testing and, later, mobile share flows. |
| `uclip pause` / `uclip resume` | Temporarily stop / resume syncing without stopping the daemon. |
| `uclip logs [-f] [-n <lines>]` | Show or follow the daemon's log file. |
| `uclip doctor [--json]` | Diagnostics (see §10.3). Exit 1 if any check fails; warnings do not fail. |
| `uclip autostart enable\|disable\|status` | Manage the per-user login item (§11.6). |
| `uclip completions <shell>` | Print completions for `bash`, `zsh`, `fish`, or `powershell`. |
| `uclip --version` | Version and build target. |

### 10.2 Output conventions and exit codes

- **stdout is for data** (`paste`, `--json`, tables); **stderr is for progress, warnings, and errors.** This is a good moment to teach `println!` vs `eprintln!`.
- Prefixes: `▸` (cyan) info, `✓` (green) success, `✗` (red) error, `⚠` (yellow) warning. Honor `NO_COLOR` and disable color when stdout is not a terminal (`std::io::IsTerminal`).
- `--json` on `status`, `devices`, and `doctor` prints machine-readable output (serialize a typed struct with `serde_json`).

| Exit code | Meaning |
|---|---|
| 0 | Success |
| 1 | General error (message on stderr) |
| 2 | Usage error (clap's default for bad arguments) |
| 3 | Daemon not running (for commands that need it) |
| 4 | Pairing failed or was rejected |

Teach `std::process::ExitCode` and why `main` returns it.

### 10.3 `doctor` checks

Implemented as a `Check` trait with one small struct per check, collected in a `Vec<Box<dyn Check>>` (Phase 9 — a natural place to teach trait objects).

1. Config and data directories exist and are writable.
2. `device.toml` parses and has an id.
3. Identity key present; permissions correct (Unix).
4. Clipboard backend is readable. **Read-only:** never overwrite the user's clipboard to test it.
5. Daemon reachable over IPC; daemon version matches the CLI.
6. Daemon is listening; mDNS advertisement is active.
7. Firewall hint (OS-specific, best effort).
8. Each paired peer: last seen / reachable (a warning when offline, not a failure).
9. Autostart installed and pointing at an existing `uclipd`.
10. Log directory writable; number of recent `ERROR` lines.

---

## 11. Daemon Lifecycle & OS Integration

### 11.1 Startup sequence of `uclipd`

1. Resolve paths; load `device.toml`, `config.toml`, `peers.toml`; initialize logging (keep the appender guard alive).
2. **Single-instance check** (§11.3).
3. Bind the IPC endpoint with restricted permissions.
4. Create the clipboard backend and start the watcher thread.
5. Start the sync engine actor, peer manager, listener, and discovery.
6. Log "ready". Wait for a shutdown trigger.

### 11.2 Shutdown

- Triggers: Ctrl-C / `SIGINT`, `SIGTERM` (Unix), Windows console control events, and the IPC `Stop` request.
- Steps: cancel the shared `CancellationToken` → tasks finish their current work → send `Goodbye` to peers → remove the socket file → flush logs → exit `0`.
- Teach `tokio::signal`, `select!`, and why each task needs a cancellation path.

### 11.3 Single instance and stale endpoints

- Try to connect to the IPC endpoint first. If someone answers, print "already running" and exit.
- On Unix, a leftover socket file from a crash makes `bind` fail with "address in use". If connecting is refused and the file exists, it is stale: remove it and bind again. This is a good exercise in matching `std::io::ErrorKind` values.
- Windows named pipes disappear with their process, so there is no stale-endpoint problem — a good "why is Windows different?" discussion.
- If connect-then-bind proves racy, evaluate a file-lock crate. Let the user hit the race before you mention it.

### 11.4 `uclip start` (detached spawn)

- Locate `uclipd` next to the running executable (`std::env::current_exe()`, then `parent()` and `join`). Teach `Path` vs `PathBuf` here.
- Spawn with `std::process::Command`, stdio redirected (null or the log file). Detach per OS: on Unix, start it in its own process group/session so a closing terminal does not kill it; on Windows use the `DETACHED_PROCESS` and `CREATE_NO_WINDOW` creation flags (via the platform-specific `CommandExt` traits). Verify the exact APIs in the `std` docs for the user's toolchain.
- Then poll the IPC endpoint (e.g. every 100 ms for up to 5 s) until it answers; report success or "did not become ready — see `uclip logs`".

### 11.5 Logging

- `tracing` events; a daily-rolling file via `tracing-appender`'s non-blocking writer. **Gotcha:** the writer returns a *guard*; if it is dropped early, buffered logs are lost. Have the user find this out by asking "why is my log file empty on exit?".
- Levels: `error` (something failed and matters), `warn` (degraded: peer dropped, clip skipped), `info` (lifecycle, pairing, connect/disconnect), `debug` (per-message metadata), `trace` (noisy).
- **Never log clipboard contents, keys, or pairing codes.** Sizes, hashes (truncated), and peer ids are fine.
- `uclip logs -f`: read the newest file and follow appended bytes — a small exercise in `File`, `Seek`, and `BufRead`.

### 11.6 Autostart templates

Provide these as boilerplate with a line-by-line explanation; write the *Rust that renders and installs them* incrementally (§2.3). Placeholders use `{{...}}`.

**macOS — LaunchAgent** at `~/Library/LaunchAgents/dev.uclip.uclipd.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>              <string>dev.uclip.uclipd</string>
    <key>ProgramArguments</key>   <array><string>{{UCLIPD_PATH}}</string></array>
    <key>RunAtLoad</key>          <true/>
    <key>KeepAlive</key>          <dict><key>SuccessfulExit</key><false/></dict>
    <key>ProcessType</key>        <string>Interactive</string>
    <key>StandardErrorPath</key>  <string>{{LOG_DIR}}/launchd.err.log</string>
</dict>
</plist>
```

Load: `launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/dev.uclip.uclipd.plist`
Unload: `launchctl bootout gui/$(id -u)/dev.uclip.uclipd`
Inspect: `launchctl print gui/$(id -u)/dev.uclip.uclipd`

**Linux — systemd user unit** at `~/.config/systemd/user/uclipd.service`:

```ini
[Unit]
Description=uclip clipboard sync daemon
PartOf=graphical-session.target
After=graphical-session.target

[Service]
ExecStart={{UCLIPD_PATH}}
Restart=on-failure

[Install]
WantedBy=graphical-session.target
```

Enable: `systemctl --user daemon-reload && systemctl --user enable --now uclipd.service`
Logs: `journalctl --user -u uclipd -f`

**Windows — per-user Run key** (simplest; verify the syntax):

```
reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v uclip /t REG_SZ /d "\"{{UCLIPD_PATH}}\"" /f
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v uclip /f
```

Alternative: a scheduled task at logon — `schtasks /Create /SC ONLOGON /TN uclip /TR "\"{{UCLIPD_PATH}}\"" /RL LIMITED /F` (delete with `schtasks /Delete /TN uclip /F`). Have the user compare the two in an ADR.

### 11.7 Gotchas to surface (let the user hit them first when safe)

- **launchd `KeepAlive: true`** restarts a daemon that exited cleanly, so `uclip stop` appears to do nothing. `SuccessfulExit = false` restarts only after crashes.
- **Console window flashing at login on Windows.** Build the daemon for the GUI subsystem: `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` at the top of `uclipd.rs`. This is also a nice first look at *attributes*. It means no stdout, so logging must go to a file.
- Paths with spaces must be quoted in every template.
- Linux user services may not inherit `DISPLAY` / `WAYLAND_DISPLAY`; import them into the user manager's environment.
- Installing a new binary does not replace the *running* daemon: `uclip restart`.
- Autostart entries hold an absolute path to `uclipd`; if the binary moves, autostart silently breaks. `doctor` must detect this.
