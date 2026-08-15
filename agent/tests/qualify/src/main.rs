#[path = "../boundary.rs"]
mod boundary;
#[path = "../fabrication.rs"]
mod fabrication;
#[path = "../hostile_node.rs"]
mod hostile_node;
#[path = "../wire.rs"]
mod wire;

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    let Some(command) = arguments.next() else {
        eprintln!("usage: agent-qualification-gates GATE [ARGS]");
        return ExitCode::FAILURE;
    };
    let result = if command == "wire" {
        let Some(repository) = arguments.next().map(PathBuf::from) else {
            eprintln!("wire gate is missing REPOSITORY");
            return ExitCode::FAILURE;
        };
        let Some(reference) = arguments.next().map(PathBuf::from) else {
            eprintln!("wire gate is missing C_REFERENCE");
            return ExitCode::FAILURE;
        };
        let Some(harness) = arguments.next().map(PathBuf::from) else {
            eprintln!("wire gate is missing HARNESS");
            return ExitCode::FAILURE;
        };
        if arguments.next().is_some() {
            Err("wire gate received an unexpected argument".to_owned())
        } else {
            wire::agent_qualify_wire_gate(&repository, &reference, &harness)
        }
    } else if command == "boundary" {
        let Some(repository) = arguments.next().map(PathBuf::from) else {
            eprintln!("boundary gate is missing REPOSITORY");
            return ExitCode::FAILURE;
        };
        let Some(node) = arguments.next().map(PathBuf::from) else {
            eprintln!("boundary gate is missing NODE");
            return ExitCode::FAILURE;
        };
        let Some(harness) = arguments.next().map(PathBuf::from) else {
            eprintln!("boundary gate is missing HARNESS");
            return ExitCode::FAILURE;
        };
        if arguments.next().is_some() {
            Err("boundary gate received an unexpected argument".to_owned())
        } else {
            boundary::agent_qualify_boundary_gate(&repository, &node, &harness)
        }
    } else if command == "fabrication" {
        let Some(repository) = arguments.next().map(PathBuf::from) else {
            eprintln!("fabrication gate is missing REPOSITORY");
            return ExitCode::FAILURE;
        };
        if arguments.next().is_some() {
            Err("fabrication gate received an unexpected argument".to_owned())
        } else {
            fabrication::agent_qualify_fabrication_gate(&repository)
        }
    } else {
        Err(format!(
            "unknown qualification gate {}",
            command.to_string_lossy()
        ))
    };
    match result {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("qualification gate failed: {error}");
            ExitCode::FAILURE
        }
    }
}
