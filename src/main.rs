mod cli;

use cli::{background_start::background_start, flash::flash, list::list, start::start, stop::stop};

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(about = "List all serial ports on system")]
    List,
    #[command(about = "Start listening to the given serial port")]
    Start {
        #[arg(short, long)]
        port: Option<String>,
        #[arg(short, long)]
        config_path: Option<String>,
        #[arg(long)]
        tcp_port: Option<String>,
        #[arg(short, long, default_value_t = false)]
        foreground: bool,
    },
    #[command(about = "Stop the running serial port listener")]
    Stop {
        #[arg(short, long)]
        tcp_port: Option<String>,
    },
    #[command(about = "Flash the icons to the device")]
    Flash {
        #[arg(short, long)]
        tcp_port: Option<String>,
        #[arg(short, long)]
        config_path: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::List => list(),
        Commands::Start {
            port,
            config_path,
            foreground,
            tcp_port,
        } => {
            if foreground {
                start(port, config_path, tcp_port);
            } else {
                background_start(port, config_path, tcp_port);
            }
        }
        Commands::Stop { tcp_port } => stop(tcp_port),
        Commands::Flash {
            tcp_port,
            config_path,
        } => flash(tcp_port, config_path),
    }
}
