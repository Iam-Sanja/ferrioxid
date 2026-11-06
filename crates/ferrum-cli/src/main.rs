//! Experimental CLI: only implemented commands are exposed.
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "ferrum",
    about = "Ferrum development tools (experimental)",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print current implementation status and setup instructions.
    Doctor,
}

fn main() {
    match Cli::parse().command {
        Command::Doctor => {
            println!("Ferrum v0.1: PostgreSQL required; set DATABASE_URL.");
            println!("Start database: docker compose up -d db");
            println!("Bootstrap tables: cargo run -p myshop -- --bootstrap");
            println!("Start server: cargo run -p myshop");
            println!("Migrations, code generation, and admin UI are NOT implemented in v0.1.");
        }
    }
}
