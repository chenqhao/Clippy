use clap::{Parser, Subcommand};
use std::fs;
use std::process::ExitCode;
use uclip::{DeviceId, DeviceInfo, Paths, Settings};

/// uclip: A universal clipboard sync tool.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Print the path to the config file.
    Path,
    /// Print the configuration contents.
    Show,
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
                    let config_content = fs::read_to_string(config_path).expect("Could not read config file");
                    let config: Settings = toml::from_str(&config_content).expect("Could not parse config file");
                    println!("{:#?}", config);  
                }
            }
            ExitCode::SUCCESS
        }
    }
}
