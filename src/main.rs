mod cli;
use cli::{list::list, start::start};

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(visible_alias = "ls")]
    #[command(about = "List all serial ports on system")]
    List,
    #[command(about = "Start listening to the given serial port")]
    Start {
        #[arg(index = 1, value_name = "PORT")]
        port: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::List => list(),
        Commands::Start { port } => start(port),
    }
}
