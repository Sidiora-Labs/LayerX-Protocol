use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use layerx_programs_runtime::{programs_fuzz_targets, FuzzTarget};

fn decode_hex(text: &str) -> Result<Vec<u8>, String> {
    let compact: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    if !compact.len().is_multiple_of(2) {
        return Err("corpus hex has odd length".to_string());
    }
    compact
        .chunks_exact(2)
        .map(|pair| {
            let high = (pair[0] as char)
                .to_digit(16)
                .ok_or_else(|| "invalid corpus hex".to_string())?;
            let low = (pair[1] as char)
                .to_digit(16)
                .ok_or_else(|| "invalid corpus hex".to_string())?;
            u8::try_from((high << 4) | low).map_err(|error| error.to_string())
        })
        .collect()
}

fn target(name: &str) -> Result<FuzzTarget, String> {
    match name {
        "validation" => Ok(FuzzTarget::Validation),
        "instantiation" => Ok(FuzzTarget::Instantiation),
        "execution" => Ok(FuzzTarget::Execution),
        _ => Err(format!("unknown fuzz target {name}")),
    }
}

fn corpus_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(root).map_err(|error| error.to_string())?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "hex"))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!("empty corpus {}", root.display()));
    }
    Ok(files)
}

fn mutate(seed: &[u8]) -> Vec<Vec<u8>> {
    let mut cases = vec![seed.to_vec(), Vec::new()];
    if !seed.is_empty() {
        for index in 0..seed.len().min(64) {
            let mut flipped = seed.to_vec();
            flipped[index] ^= 0xff;
            cases.push(flipped);
        }
        cases.push(seed[..seed.len() / 2].to_vec());
    }
    cases
}

fn run() -> Result<(), String> {
    let mut arguments = env::args().skip(1);
    let name = arguments
        .next()
        .ok_or_else(|| "missing fuzz target".to_string())?;
    let corpus = arguments
        .next()
        .ok_or_else(|| "missing corpus directory".to_string())?;
    if arguments.next().is_some() {
        return Err("unexpected fuzz argument".to_string());
    }
    let selected = target(&name)?;
    for path in corpus_files(Path::new(&corpus))? {
        let source = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let seed = decode_hex(&source)?;
        for input in mutate(&seed) {
            programs_fuzz_targets(selected, &input);
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("programs fuzz failed: {error}");
        std::process::exit(1);
    }
}
