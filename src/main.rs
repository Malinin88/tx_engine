use std::env;
use std::fs::File;
use std::io::{self, BufReader};
use std::process;

use tx_engine::Engine;
use tx_engine::error::{EngineError, ParseError};
use tx_engine::io::{read_transactions, write_clients};

fn run() -> Result<(), EngineError> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <transactions.csv>", args[0]);
        process::exit(1);
    }

    let file = File::open(&args[1]).map_err(EngineError::Io)?;
    let reader = BufReader::new(file);
    let mut engine = Engine::new();

    for result in read_transactions(reader) {
        match result {
            Ok((tx, _row)) => {
                if let Err(engine_err) = engine.process(tx) {
                    eprintln!("warning: {engine_err}");
                }
            }
            Err(ParseError::Io(e)) => {
                return Err(EngineError::Io(e));
            }
            Err(ParseError::Csv(e)) => {
                return Err(EngineError::Io(io::Error::other(e)));
            }
            Err(parse_err) => {
                eprintln!("warning: {parse_err}");
            }
        }
    }

    let stdout = io::stdout();
    write_clients(stdout.lock(), engine.snapshots())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}
