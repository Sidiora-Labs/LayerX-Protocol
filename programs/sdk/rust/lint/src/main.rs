//! Build-time determinism gate for `LayerX` program projects.
//!
//! The lint is a build step, not a report: it exits non-zero and names the
//! violated rule, so a program that reaches for a clock, for randomness, for
//! floating point or for a host interface outside the frozen `layerx_v1`
//! surface fails its build before deployment ever sees it.

use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use layerx_program_lint::{
    abi_surface_violations, lint_artifact, lint_project, DeterminismViolation,
};

const USAGE: &str = "usage: layerx-program-lint <project-directory> [artifact.wasm]\n       layerx-program-lint --artifact <artifact.wasm>\n       layerx-program-lint --abi";
const USAGE_STATUS: u8 = 2;

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let requested: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let violations = match requested.as_slice() {
        ["--abi"] => abi_surface_violations(),
        ["--artifact", artifact] => artifact_violations(Path::new(artifact)),
        [project] => lint_project(Path::new(project), None),
        [project, artifact] => lint_project(Path::new(project), Some(Path::new(artifact))),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(USAGE_STATUS);
        }
    };
    if violations.is_empty() {
        return ExitCode::SUCCESS;
    }
    for violation in &violations {
        eprintln!("determinism violation [{}]: {violation}", violation.name());
    }
    ExitCode::FAILURE
}

fn artifact_violations(artifact: &Path) -> Vec<DeterminismViolation> {
    match fs::read(artifact) {
        Ok(wasm) => lint_artifact(&wasm),
        Err(error) => vec![DeterminismViolation::UnreadablePath {
            path: artifact.to_path_buf(),
            reason: error.to_string(),
        }],
    }
}
