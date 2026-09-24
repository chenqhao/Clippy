use clap::{Parser, Subcommand};
use std::fs;
use std::io;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;
use uclip::{
    ClipContent, ClipEvent, DeviceId, DeviceInfo, Paths, PollingWatcher, Settings, create_backend,
};

/// uclip: A universal clipboard sync tool.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Print the path to the config file.
    Path,
    /// Print the configuration contents.
    Show,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize the configuration and identity for this device.
    Init {
        /// Optional human-readable name for this device.
        #[arg(long)]
        name: Option<String>,

        /// Force initialization, regenerating the identity key (unpairs all peers).
        #[arg(long)]
        force: bool,
    },

    /// View configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Copy text from stdin to the clipboard.
    Copy { text: Option<String> },

    /// Paste clipboard contents to stdout.
    Paste,

    /// Watches for change in clipboard
    Watch {
        #[arg(long)]
        show: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name, force } => {
            let paths = match Paths::new() {
                Some(p) => p,
                None => {
                    eprintln!("✗ Could not determine home directory for your OS.");
                    return ExitCode::FAILURE;
                }
            };

            // 1. Create the directories
            if let Err(e) = fs::create_dir_all(paths.config_dir()) {
                eprintln!("✗ Failed to create config directory: {}", e);
                return ExitCode::FAILURE;
            }
            if let Err(e) = fs::create_dir_all(paths.data_dir()) {
                eprintln!("✗ Failed to create data directory: {}", e);
                return ExitCode::FAILURE;
            }
            println!("▸ Created directories.");

            // 2. Create the settings file if it doesn't exist
            let config_path = paths.config_file();
            if !config_path.exists() || force {
                let default_settings = Settings::default();
                let toml_str = toml::to_string_pretty(&default_settings).unwrap();
                fs::write(&config_path, toml_str).unwrap();
                println!("✓ Wrote default config to {:?}", config_path);
            } else {
                println!("▸ Config file already exists, skipping.");
            }

            // 3. Create the device identity file if it doesn't exist (or --force)
            let identity_path = paths.identity_file();
            if !identity_path.exists() || force {
                if force && identity_path.exists() {
                    println!(
                        "⚠ --force was used: regenerating identity. You will need to re-pair with all peers."
                    );
                }

                // If they didn't provide a name, ask the OS for the computer's hostname!
                let device_name = name
                    .unwrap_or_else(|| gethostname::gethostname().to_string_lossy().into_owned());

                let info = DeviceInfo {
                    id: DeviceId::new(),
                    name: device_name,
                };

                let toml_str = toml::to_string_pretty(&info).unwrap();
                fs::write(&identity_path, toml_str).unwrap();
                println!("✓ Generated new device identity: {}", info.id);
            } else {
                println!("▸ Device identity already exists, skipping.");
            }

            println!("✓ Initialization complete.");
            ExitCode::SUCCESS
        }

        Commands::Config { action } => {
            match action {
                ConfigAction::Path => {
                    let paths = Paths::new().expect("Could not determine paths");
                    let config_path = paths.config_file();
                    println!("▸ Config path: {}", config_path.display());
                }
                ConfigAction::Show => {
                    let paths = Paths::new().expect("Could not determine paths");
                    let config_path = paths.config_file();
                    let config_content =
                        fs::read_to_string(config_path).expect("Could not read config file");
                    let config: Settings =
                        toml::from_str(&config_content).expect("Could not parse config file");
                    println!("{:#?}", config);
                }
            }
            ExitCode::SUCCESS
        }

        Commands::Copy { text } => {
            // 1. Read all of stdin into a String.
            let text = match text {
                Some(t) => {
                    eprintln!("Copied to clipboard: {t}");
                    t
                }
                None => {
                    eprintln!(
                        "Type your text, hit enter and then hit Ctrl + D to copy to clipboard"
                    );
                    match io::read_to_string(io::stdin()) {
                        Ok(t) => {
                            eprintln!("Copied to clipboard: {t}");
                            t
                        }
                        Err(e) => {
                            eprintln!("✗ failed to read stdin: {e}");
                            return ExitCode::FAILURE;
                        }
                    }
                }
            };

            // 2. Open the clipboard backend.
            let mut clipboard = match create_backend() {
                Ok(cb) => cb,
                Err(e) => {
                    eprintln!("✗ {e}");
                    return ExitCode::FAILURE;
                }
            };

            // 3. Set the clipboard content.
            let content = ClipContent::Text { text };
            if let Err(e) = clipboard.set(&content) {
                eprintln!("✗ {e}");
                return ExitCode::FAILURE;
            }

            ExitCode::SUCCESS
        }

        Commands::Paste => {
            // 1. Open the clipboard backend.
            let mut clipboard = match create_backend() {
                Ok(cb) => cb,
                Err(e) => {
                    eprintln!("✗ {e}");
                    return ExitCode::FAILURE;
                }
            };

            // 2. Read the clipboard content.
            match clipboard.get() {
                Ok(Some(ClipContent::Text { text })) => {
                    // Print without a trailing newline — this lets
                    // `uclip paste | wc -c` count the exact bytes.
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Ok(Some(_)) => {
                    // Non-text content (e.g. image in Phase 10)
                    eprintln!("✗ clipboard contains non-text content");
                    ExitCode::FAILURE
                }
                Ok(None) => {
                    // Clipboard is empty or has non-text content.
                    // Exit silently with a non-zero code (convention).
                    ExitCode::FAILURE
                }
                Err(e) => {
                    eprintln!("✗ {e}");
                    ExitCode::FAILURE
                }
            }
        }

        Commands::Watch { show } => {
            // 1. Open the clipboard backend.
            let clipboard = match create_backend() {
                Ok(cb) => cb,
                Err(e) => {
                    eprintln!("✗ {e}");
                    return ExitCode::FAILURE;
                }
            };

            // 2. Wrap it in our polling watcher.
            let mut watcher = PollingWatcher::new(clipboard);

            // 3. Create a channel to send events from worker thread to main thread.
            let (tx, rx) = mpsc::channel();

            // 4. Create an atomic stop flag shared across threads.
            let running = Arc::new(AtomicBool::new(true));

            // 5. Register Ctrl-C handler to trigger clean shutdown.
            let r = Arc::clone(&running);
            if let Err(e) = ctrlc::set_handler(move || {
                r.store(false, Ordering::SeqCst);
            }) {
                eprintln!("✗ failed to set Ctrl-C handler: {e}");
                return ExitCode::FAILURE;
            }

            // 6. Spawn the background worker thread.
            let stop = Arc::clone(&running);
            let handle = thread::spawn(move || {
                while stop.load(Ordering::Relaxed) {
                    match watcher.poll_once() {
                        Ok(Some(event)) => {
                            if tx.send(event).is_err() {
                                break;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => eprintln!("✗ {e}"),
                    }
                    thread::sleep(Duration::from_millis(500));
                }
            });

            // 7. Consume events with a timeout so we regularly check the `running` flag.
            while running.load(Ordering::Relaxed) {
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(event) => match event {
                        ClipEvent::Changed(content) => {
                            let hash = content.content_hash();
                            let short_hash = format!(
                                "{:02x}{:02x}{:02x}{:02x}",
                                hash[0], hash[1], hash[2], hash[3]
                            );

                            if let ClipContent::Text { text } = content {
                                let len = text.len();
                                let unit = if len == 1 { "byte" } else { "bytes" };
                                println!("▸ clip changed: {short_hash} (text, {len} {unit})");

                                if show {
                                    for line in text.lines() {
                                        println!("  {line}");
                                    }
                                }
                            }
                        }
                    },
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // Timeout: loop back and check `running`
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }

            // 8. Clean shutdown: wait for the worker thread to exit.
            eprintln!("\n▸ Stopping clipboard watcher...");

            match handle.join() {
                Ok(_) => eprintln!("✔ Watcher stopped."),
                Err(e) => eprintln!("✗ failed to join watcher thread: {e:?}"),
            }
            ExitCode::SUCCESS
        }
    }
}
