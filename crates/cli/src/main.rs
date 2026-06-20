use ss_core::config::Config;
use ss_core::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ss-cli")]
#[command(about = "Screen Sharing Service CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Setup {
        #[arg(short, long)]
        name: Option<String>,
    },
    Consent {
        #[command(subcommand)]
        action: ConsentAction,
    },
    Status,
    Uninstall,
    GenerateTls,
}

#[derive(Subcommand)]
enum ConsentAction {
    Grant,
    Revoke,
    Check,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Setup { name } => setup(name),
        Commands::Consent { action } => match action {
            ConsentAction::Grant => grant_consent(),
            ConsentAction::Revoke => revoke_consent(),
            ConsentAction::Check => check_consent(),
        },
        Commands::Status => show_status(),
        Commands::Uninstall => uninstall(),
        Commands::GenerateTls => generate_tls(),
    }
}

fn setup(name: Option<String>) -> Result<()> {
    println!("=== Screen Sharing Service Setup ===");
    println!();

    let device_name = name.unwrap_or_else(|| {
        hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    });

    println!("Device name: {}", device_name);
    println!();

    println!("This will create a configuration file at:");
    println!("  {}", Config::config_path().display());
    println!();

    print!("Do you want to proceed? [y/N]: ");
    use std::io::Write;
    std::io::stdout().flush().unwrap();

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    if input.trim().to_lowercase() != "y" {
        println!("Setup cancelled.");
        return Ok(());
    }

    let mut config = Config::default();
    config.device.name = device_name;
    config.save()?;

    println!();
    println!("Configuration created successfully!");
    println!();
    println!("Next steps:");
    println!("  1. Grant consent: ss-cli consent grant");
    println!("  2. Generate TLS certificates: ss-cli generate-tls");
    println!("  3. Install the service: sc create SSService binPath= \"{}\" start= auto", 
        std::env::current_exe()?.display());
    println!("  4. Start the service: sc start SSService");

    Ok(())
}

fn grant_consent() -> Result<()> {
    println!("Granting consent for screen sharing...");
    println!();
    println!("WARNING: This will allow the service to capture and stream your screen.");
    println!("Only authorized viewers will be able to watch.");
    println!();

    print!("Do you consent? [y/N]: ");
    use std::io::Write;
    std::io::stdout().flush().unwrap();

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    if input.trim().to_lowercase() != "y" {
        println!("Consent not granted.");
        return Ok(());
    }

    let config = Config::load()?;
    let consent = ss_core::ConsentManager::new()?;
    consent.grant_consent(&config.device.id)?;

    println!();
    println!("Consent granted successfully!");
    println!("The service can now capture and stream your screen.");

    Ok(())
}

fn revoke_consent() -> Result<()> {
    println!("Revoking consent...");

    let consent = ss_core::ConsentManager::new()?;
    consent.revoke_consent()?;

    println!("Consent revoked.");
    println!("The service will no longer be able to capture your screen.");

    Ok(())
}

fn check_consent() -> Result<()> {
    let config = Config::load()?;
    let consent = ss_core::ConsentManager::new()?;

    if consent.is_consent_granted(&config.device.id)? {
        println!("Consent is granted.");
    } else {
        println!("Consent is not granted.");
    }

    Ok(())
}

fn show_status() -> Result<()> {
    println!("=== Screen Sharing Service Status ===");
    println!();

    if Config::config_path().exists() {
        let config = Config::load()?;
        println!("Device ID: {}", config.device.id);
        println!("Device Name: {}", config.device.name);
        println!("HTTP Port: {}", config.server.http_port);
        println!("HTTPS Port: {}", config.server.https_port);
        println!("Max Viewers: {}", config.server.max_viewers);

        let consent = ss_core::ConsentManager::new()?;
        if consent.is_consent_granted(&config.device.id)? {
            println!("Consent: Granted");
        } else {
            println!("Consent: Not granted");
        }
    } else {
        println!("Not configured. Run 'ss-cli setup' first.");
    }

    Ok(())
}

fn uninstall() -> Result<()> {
    println!("=== Uninstalling Screen Sharing Service ===");
    println!();

    print!("This will remove the service and all configuration. Continue? [y/N]: ");
    use std::io::Write;
    std::io::stdout().flush().unwrap();

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    if input.trim().to_lowercase() != "y" {
        println!("Uninstall cancelled.");
        return Ok(());
    }

    println!("Stopping service...");
    std::process::Command::new("sc")
        .args(["stop", "SSService"])
        .output()
        .ok();

    println!("Deleting service...");
    std::process::Command::new("sc")
        .args(["delete", "SSService"])
        .output()
        .ok();

    println!("Removing configuration...");
    let config_dir = Config::config_dir();
    if config_dir.exists() {
        std::fs::remove_dir_all(&config_dir)?;
    }

    println!();
    println!("Service uninstalled successfully.");

    Ok(())
}

fn generate_tls() -> Result<()> {
    println!("Generating TLS certificates...");

    ss_server::tls::TlsConfig::generate_self_signed()?;

    println!("TLS certificates generated at:");
    println!("  {}", Config::tls_dir().join("cert.pem").display());
    println!("  {}", Config::tls_dir().join("key.pem").display());

    Ok(())
}
