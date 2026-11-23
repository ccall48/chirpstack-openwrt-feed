use std::path::PathBuf;
use std::process::exit;

use anyhow::Result;

mod collectd;
mod daemon;
mod proto;
mod state;

const DEFAULT_SOCKET_DIR: &str = "/tmp";
const DEFAULT_STATE_DIR: &str = "/var/run/chirpstack-lorawan-stats";

const USAGE: &str = "Usage: chirpstack-lorawan-stats <COMMAND> [OPTIONS]

Commands:
  daemon     Subscribe to all Concentratord event sockets and write the state
             files (must run as a user that can access the Concentratord sockets)
  collectd   Print the state files using the collectd exec plugin protocol
  print      Print the state files

Options:
  --socket-dir <DIR>  Directory containing the Concentratord IPC sockets [default: /tmp]
  --state-dir <DIR>   Directory containing the state files [default: /var/run/chirpstack-lorawan-stats]
";

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {:#}", e);
        exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut command: Option<String> = None;
    let mut socket_dir = PathBuf::from(DEFAULT_SOCKET_DIR);
    let mut state_dir = PathBuf::from(DEFAULT_STATE_DIR);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--socket-dir" => socket_dir = args.next().map(PathBuf::from).unwrap_or(socket_dir),
            "--state-dir" => state_dir = args.next().map(PathBuf::from).unwrap_or(state_dir),
            "-h" | "--help" => {
                print!("{}", USAGE);
                return Ok(());
            }
            "-V" | "--version" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ if command.is_none() && !arg.starts_with('-') => command = Some(arg),
            _ => {
                eprint!("Unexpected argument: {}\n\n{}", arg, USAGE);
                exit(2);
            }
        }
    }

    match command.as_deref() {
        Some("daemon") => daemon::run(&socket_dir, &state_dir),
        Some("collectd") => collectd::run(&state_dir),
        Some("print") => print(&state_dir),
        _ => {
            eprint!("{}", USAGE);
            exit(2);
        }
    }
}

fn print(state_dir: &std::path::Path) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(state_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| !e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for e in entries {
        println!("[{}]", e.file_name().to_string_lossy());
        print!("{}", std::fs::read_to_string(e.path())?);
    }

    Ok(())
}
