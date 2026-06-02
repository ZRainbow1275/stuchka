//! `stuchka-eval` — run one evaluation gate over its dataset and emit a JSON report.
//!
//! Usage:
//!   stuchka-eval <gate> <dataset.json> [--report <out.json>] [--pretty]
//!
//! `<gate>` is one of: calc | deadline | pii | doc | law. The process exits 0 once it has run and
//! written the report (threshold judgement is the Python `packages/eval-runner`'s job); it exits 2
//! only on a genuine error (bad args, unreadable dataset, engine init failure).

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: stuchka-eval <gate> <dataset.json> [--report <out.json>] [--pretty]");
        eprintln!("       gate = calc | deadline | pii | doc | law");
        return ExitCode::from(2);
    }
    let gate = args[1].clone();
    let dataset = PathBuf::from(&args[2]);

    let mut report_path: Option<PathBuf> = None;
    let mut pretty = false;
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--report" => {
                if i + 1 >= args.len() {
                    eprintln!("--report requires a path");
                    return ExitCode::from(2);
                }
                report_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--pretty" => {
                pretty = true;
                i += 1;
            }
            other => {
                eprintln!("unknown argument '{other}'");
                return ExitCode::from(2);
            }
        }
    }

    match eval_cli::run_gate(&gate, &dataset) {
        Ok(report) => {
            let json = if pretty {
                serde_json::to_string_pretty(&report)
            } else {
                serde_json::to_string(&report)
            };
            let json = match json {
                Ok(j) => j,
                Err(e) => {
                    eprintln!("serialize report: {e}");
                    return ExitCode::from(2);
                }
            };
            if let Some(path) = report_path {
                if let Err(e) = std::fs::write(&path, json.as_bytes()) {
                    eprintln!("write report {}: {e}", path.display());
                    return ExitCode::from(2);
                }
            } else {
                println!("{json}");
            }
            eprintln!("{}", report.summary_line());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("eval error: {e}");
            ExitCode::from(2)
        }
    }
}
