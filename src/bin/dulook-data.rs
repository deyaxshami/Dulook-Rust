//! Developer tool for the offline Hadith collection.
//!
//! `dulook-data validate`       validate data/hadith-fallback.json
//! `dulook-data build`          rebuild it from the pinned upstream editions
//! `dulook-data build --check`  verify the committed file is reproducible

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use dulook::hadith_data::{SELECTION_MANIFEST, build_hadith_fallback, validate_hadith_data};
use serde_json::Value;

const DATA_FILE: &str = "data/hadith-fallback.json";
const USAGE: &str = "usage: dulook-data validate | build [--check]";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn curl(edition: &str, url: &str) -> Result<Vec<u8>, String> {
    let output = Command::new("/usr/bin/curl")
        .args(["-fLsS", url])
        .output()
        .map_err(|e| format!("Failed to fetch {edition}: {e}"))?;
    if !output.status.success() {
        let code = output.status.code().map_or_else(|| "signal".to_owned(), |c| c.to_string());
        return Err(format!("Failed to fetch {edition}: curl exit {code}"));
    }
    Ok(output.stdout)
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let root = root();
    let manifest = read_json(&root.join(SELECTION_MANIFEST))?;
    let data_path = root.join(DATA_FILE);
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["validate"] => {
            let summary = validate_hadith_data(&read_json(&data_path)?, &manifest)?;
            println!(
                "Validated {} bilingual Hadith records ({} Bukhari, {} Muslim).",
                summary.total, summary.bukhari, summary.muslim
            );
        }
        ["build"] => {
            let (serialized, count) = build_hadith_fallback(&manifest, curl)?;
            std::fs::write(&data_path, serialized).map_err(|e| format!("{}: {e}", data_path.display()))?;
            println!("Wrote {count} records to {DATA_FILE}.");
        }
        ["build", "--check"] => {
            let (serialized, count) = build_hadith_fallback(&manifest, curl)?;
            let current = std::fs::read_to_string(&data_path).map_err(|e| format!("{}: {e}", data_path.display()))?;
            if current != serialized {
                eprintln!("{DATA_FILE} is not reproducible from the pinned selection");
                return Ok(ExitCode::FAILURE);
            }
            println!("Verified deterministic Hadith fallback ({count} records).");
        }
        _ => {
            eprintln!("{USAGE}");
            return Ok(ExitCode::from(2));
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args).unwrap_or_else(|message| {
        eprintln!("{message}");
        ExitCode::FAILURE
    })
}
