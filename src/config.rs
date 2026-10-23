use std::{env::Args, process::exit};

use self::help::print_help;

mod file;
mod help;

pub use file::{Colors, NotesHeight, Reopen, Settings};

pub struct Config {
    pub query: Option<String>,
    pub command: String,
    pub settings: Settings,
}

impl Config {
    pub fn new(args: Args) -> Self {
        let mut query: Option<String> = None;
        let mut command = None;

        for arg in args.skip(1) {
            match arg.as_str() {
                "-h" | "--help" => print_help(),
                #[allow(clippy::option_if_let_else)]
                _ => {
                    if let Some(stripped) = arg.strip_prefix("-p=") {
                        command = Some(stripped.to_string());
                    } else if let Some(q) = query {
                        query = Some(q + " " + &arg);
                    } else {
                        query = Some(arg.to_owned());
                    }
                }
            }
        }

        let mut settings = match file::load() {
            Ok(settings) => settings,
            Err(err) => {
                eprintln!("parui: {err}");
                exit(1);
            }
        };

        // `-p=` takes precedence over the configuration file.
        let command = command
            .or_else(|| settings.program.take())
            .unwrap_or_else(|| String::from("paru"));

        if let Err(err) = std::process::Command::new(&command).arg("--help").output() {
            match err.kind() {
                std::io::ErrorKind::NotFound => {
                    eprintln!("parui: {command}: command not found");
                }
                _ => {
                    eprintln!("parui: {command}: {err}");
                }
            }
            exit(1);
        }

        Self {
            query,
            command,
            settings,
        }
    }
}
