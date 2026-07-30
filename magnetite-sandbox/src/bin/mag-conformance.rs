//! `mag-conformance` — check any wasm module against the public `mag_*` ABI.
//!
//! ```text
//! mag-conformance <module.wasm|module.wat> [--ticks N] [--seed N] [--players N]
//!                                          [--fuel N] [--memory-mib N] [--json]
//! ```
//!
//! Exit status is 0 when no check failed, 1 when any check failed, 2 on a usage
//! or I/O error. The contract the checks are drawn from is `site/docs/sandbox-abi.md`.

use std::process::ExitCode;

use magnetite_sandbox::conformance::{self, ConformanceOptions, Status};

const USAGE: &str = "\
mag-conformance — validate a wasm module against the magnetite mag_* sandbox ABI

USAGE:
    mag-conformance <MODULE> [OPTIONS]

ARGS:
    <MODULE>            Path to a .wasm module, or a .wat text module

OPTIONS:
    --ticks N           Ticks to step for the determinism and replay checks [24]
    --seed N            MatchConfig.seed handed to the module
    --players N         Synthetic players whose inputs are submitted each tick [4]
    --fuel N            Fuel units granted per mag_step [10000000]
    --memory-mib N      Guest linear-memory cap in MiB [64]
    --json              Emit machine-readable JSON instead of a table
    -h, --help          Print this help

EXIT STATUS:
    0  every check passed (warnings do not fail a run)
    1  at least one check failed
    2  usage or I/O error
";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut path: Option<String> = None;
    let mut json = false;
    let mut opts = ConformanceOptions::default();
    let mut limits = opts.limits.clone();

    while let Some(arg) = args.next() {
        let mut value = |name: &str| -> Option<String> {
            match args.next() {
                Some(v) => Some(v),
                None => {
                    eprintln!("error: {name} needs a value");
                    None
                }
            }
        };
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--json" => json = true,
            "--ticks" => match value("--ticks").and_then(|v| v.parse().ok()) {
                Some(v) => opts.ticks = v,
                None => return ExitCode::from(2),
            },
            "--seed" => match value("--seed").and_then(|v| v.parse().ok()) {
                Some(v) => opts.seed = v,
                None => return ExitCode::from(2),
            },
            "--players" => match value("--players").and_then(|v| v.parse().ok()) {
                Some(v) => opts.players = v,
                None => return ExitCode::from(2),
            },
            "--fuel" => match value("--fuel").and_then(|v| v.parse().ok()) {
                Some(v) => limits.fuel_per_step = v,
                None => return ExitCode::from(2),
            },
            "--memory-mib" => match value("--memory-mib").and_then(|v| v.parse::<usize>().ok()) {
                Some(v) => limits.max_memory_bytes = v * 1024 * 1024,
                None => return ExitCode::from(2),
            },
            other if other.starts_with('-') => {
                eprintln!("error: unknown option `{other}`\n\n{USAGE}");
                return ExitCode::from(2);
            }
            other => {
                if path.replace(other.to_string()).is_some() {
                    eprintln!("error: more than one module path given\n\n{USAGE}");
                    return ExitCode::from(2);
                }
            }
        }
    }

    let Some(path) = path else {
        eprint!("{USAGE}");
        return ExitCode::from(2);
    };

    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: cannot read {path}: {e}");
            return ExitCode::from(2);
        }
    };

    opts.limits = limits;
    let report = conformance::run(&bytes, &opts);

    if json {
        println!("{}", render_json(&path, &report));
    } else {
        println!("mag_* ABI conformance — {path}\n");
        print!("{}", report.render());
    }

    if report.is_conforming() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// Hand-rolled JSON so the binary adds no dependency the library does not need.
fn render_json(path: &str, report: &conformance::Report) -> String {
    let checks: Vec<String> = report
        .checks
        .iter()
        .map(|c| {
            format!(
                r#"{{"id":{},"status":"{}","detail":{}}}"#,
                json_str(&c.id),
                c.status.label(),
                json_str(&c.detail)
            )
        })
        .collect();
    format!(
        r#"{{"module":{},"module_bytes":{},"conforming":{},"pass":{},"fail":{},"warn":{},"info":{},"skip":{},"checks":[{}]}}"#,
        json_str(path),
        report.module_bytes,
        report.is_conforming(),
        report.count(Status::Pass),
        report.count(Status::Fail),
        report.count(Status::Warn),
        report.count(Status::Info),
        report.count(Status::Skip),
        checks.join(",")
    )
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
