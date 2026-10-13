use serde::Serialize;
use siediff::{
    compare::{self, Mode, Policy},
    control,
    model::{Encoding, Limits, Period, Status},
    parse, snapshot, strict_json, validate, Error, Result,
};
use std::{
    collections::BTreeMap,
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    process::ExitCode,
};
fn usage() -> &'static str {
    r#"SIEDiff 0.2.0 - offline observer, never a ledger writer
  siediff inspect FILE [--encoding cp437|utf8|windows1252] [--format sie|snapshot]
  siediff validate FILE [--encoding ...] [--format ...]
  siediff diff LEFT RIGHT [--mode identity|renumbered] [--period YYYYMMDD:YYYYMMDD]
      [--left-encoding ...] [--right-encoding ...] [--left-format sie|snapshot]
      [--right-format sie|snapshot] [--mapping FILE] [--work-basis FILE] [--claim supplied_postings|declared_balances|retained_history|native_relationships|migration_readiness]
  siediff controls LEDGER MANIFEST [--encoding ...] [--format ...]
Output: summary on stdout. --json explicitly permits full private JSON on stdout.
--output NEW_FILE writes full private JSON without replacing an existing file.
Exit: 0 selected assertion passes; 1 fails; 2 invalid input; 3 indeterminate/not applicable;
      4 resource limit; 5 I/O error. No exit status authorizes posting or migration.
"#
}
fn read(path: &str, max: usize) -> Result<Vec<u8>> {
    if path.contains("://") {
        return Err(Error::new(
            "INPUT_PATH",
            "Network locations are unsupported",
        ));
    }
    let meta = fs::symlink_metadata(path)
        .map_err(|_| Error::new("IO_READ", "Cannot inspect input file"))?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(Error::new(
            "INPUT_FILE",
            "Input must be a regular non-symlink file",
        ));
    }
    if meta.len() > max as u64 {
        return Err(Error::new("LIMIT_BYTES", "Input exceeds byte limit"));
    }
    let mut f = File::open(path).map_err(|_| Error::new("IO_READ", "Cannot open input file"))?;
    let opened = f
        .metadata()
        .map_err(|_| Error::new("IO_READ", "Cannot inspect opened file"))?;
    if !opened.is_file() {
        return Err(Error::new(
            "INPUT_FILE",
            "Opened input is not a regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.dev() != opened.dev() || meta.ino() != opened.ino() {
            return Err(Error::new(
                "INPUT_CHANGED",
                "Input identity changed while opening",
            ));
        }
    }
    let mut data = Vec::new();
    (&mut f)
        .take(max as u64 + 1)
        .read_to_end(&mut data)
        .map_err(|_| Error::new("IO_READ", "Cannot read input"))?;
    if data.len() > max {
        return Err(Error::new("LIMIT_BYTES", "Input exceeds byte limit"));
    }
    let after = f
        .metadata()
        .map_err(|_| Error::new("IO_READ", "Cannot recheck input"))?;
    if opened.len() != after.len()
        || data.len() as u64 != opened.len()
        || opened.modified().ok() != after.modified().ok()
    {
        return Err(Error::new(
            "INPUT_CHANGED",
            "Input changed while being read",
        ));
    }
    Ok(data)
}
fn write_new(path: &str, bytes: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(path).map_err(|_| {
        Error::new(
            "IO_OUTPUT",
            "Cannot create new output; existing files are never overwritten",
        )
    })?;
    if f.write_all(bytes).and_then(|_| f.sync_all()).is_err() {
        let _ = fs::remove_file(path);
        return Err(Error::new("IO_OUTPUT", "Output write did not complete"));
    }
    Ok(())
}
fn emit<T: Serialize>(
    value: &T,
    summary: serde_json::Value,
    flags: &BTreeMap<String, String>,
) -> Result<()> {
    let full = serde_json::to_vec_pretty(value)
        .map_err(|_| Error::new("SERIALIZE", "Cannot serialize report"))?;
    if let Some(path) = flags.get("--output") {
        write_new(path, &full)?;
    }
    let mut stdout = std::io::stdout().lock();
    let text = if flags.contains_key("--json") {
        full
    } else {
        serde_json::to_vec_pretty(&summary)
            .map_err(|_| Error::new("SERIALIZE", "Cannot serialize summary"))?
    };
    stdout
        .write_all(&text)
        .and_then(|_| stdout.write_all(b"\n"))
        .map_err(|_| Error::new("IO_OUTPUT", "Cannot write output"))?;
    Ok(())
}
fn encoding(s: Option<&String>) -> Result<Encoding> {
    match s.map(String::as_str).unwrap_or("cp437") {
        "cp437" => Ok(Encoding::Cp437),
        "utf8" => Ok(Encoding::Utf8),
        "windows1252" => Ok(Encoding::Windows1252),
        _ => Err(Error::new("ARG_ENCODING", "Unsupported encoding")),
    }
}
fn input(
    path: &str,
    format: Option<&String>,
    enc: Encoding,
    l: &Limits,
) -> Result<siediff::model::Document> {
    let bytes = read(path, l.bytes)?;
    match format.map(String::as_str).unwrap_or("sie") {
        "sie" => parse::parse(&bytes, enc, l),
        "snapshot" => snapshot::parse(&bytes, l),
        _ => Err(Error::new("ARG_FORMAT", "Unsupported input format")),
    }
}
fn status_code(s: Status) -> u8 {
    match s {
        Status::Pass => 0,
        Status::Fail => 1,
        _ => 3,
    }
}
fn run() -> Result<u8> {
    let args: Vec<String> = env::args_os()
        .skip(1)
        .map(|s| {
            s.into_string()
                .map_err(|_| Error::new("ARG_ENCODING", "Command arguments must be valid Unicode"))
        })
        .collect::<Result<Vec<_>>>()?;
    if args.is_empty() || args[0] == "--help" || args[0] == "help" {
        print!("{}", usage());
        return Ok(0);
    }
    if args[0] == "--version" {
        println!("siediff {} {}", siediff::VERSION, siediff::SEMANTICS);
        return Ok(0);
    }
    let command = &args[0];
    let mut positional = vec![];
    let mut flags = BTreeMap::new();
    let mut i = 1;
    let allowed = [
        "--json",
        "--output",
        "--encoding",
        "--format",
        "--left-encoding",
        "--right-encoding",
        "--left-format",
        "--right-format",
        "--mode",
        "--period",
        "--work-basis",
        "--mapping",
        "--claim",
    ];
    while i < args.len() {
        let arg = &args[i];
        if arg.starts_with("--") {
            if !allowed.contains(&arg.as_str()) || flags.contains_key(arg) {
                return Err(Error::new("ARGUMENT", "Unknown or repeated option"));
            }
            if arg == "--json" {
                flags.insert(arg.clone(), "true".into());
                i += 1;
            } else {
                let v = args
                    .get(i + 1)
                    .filter(|s| !s.starts_with("--"))
                    .ok_or_else(|| Error::new("ARGUMENT", "Option requires a value"))?;
                flags.insert(arg.clone(), v.clone());
                i += 2;
            }
        } else {
            positional.push(arg.clone());
            i += 1;
        }
    }
    let command_flags: &[&str] = match command.as_str() {
        "inspect" | "validate" | "controls" => &["--json", "--output", "--encoding", "--format"],
        "diff" => &[
            "--json",
            "--output",
            "--left-encoding",
            "--right-encoding",
            "--left-format",
            "--right-format",
            "--mode",
            "--period",
            "--work-basis",
            "--mapping",
            "--claim",
        ],
        _ => &[],
    };
    if flags.keys().any(|k| !command_flags.contains(&k.as_str())) {
        return Err(Error::new(
            "ARGUMENT",
            "Option does not apply to this subcommand",
        ));
    }
    let l = Limits::default();
    if let Some(o) = flags.get("--output") {
        for p in &positional {
            if Path::new(o) == Path::new(p) {
                return Err(Error::new("IO_OUTPUT", "Output cannot be an input"));
            }
        }
    }
    match command.as_str() {
        "inspect" | "validate" => {
            if positional.len() != 1 {
                return Err(Error::new("ARGUMENT", "Expected one input"));
            }
            let d = input(
                &positional[0],
                flags.get("--format"),
                encoding(flags.get("--encoding"))?,
                &l,
            )?;
            let v = validate::validate(&d, &l)?;
            let summary = serde_json::json!({"schema":"siediff.summary.v1","selected_assertion":"supported_profile","status":v.status,"vouchers":v.totals.vouchers,"final_rows":v.totals.final_rows,"history_rows":v.totals.history_rows,"findings":v.findings.iter().map(|f|(&f.code,f.status)).collect::<Vec<_>>(),"accountant_acceptance":"indeterminate"});
            if command == "inspect" {
                emit(&d, summary, &flags)?
            } else {
                emit(&v, summary, &flags)?
            }
            Ok(status_code(v.status))
        }
        "diff" => {
            if positional.len() != 2 {
                return Err(Error::new("ARGUMENT", "Expected two inputs"));
            }
            let a = input(
                &positional[0],
                flags.get("--left-format"),
                encoding(flags.get("--left-encoding"))?,
                &l,
            )?;
            let b = input(
                &positional[1],
                flags.get("--right-format"),
                encoding(flags.get("--right-encoding"))?,
                &l,
            )?;
            let mode = match flags
                .get("--mode")
                .map(String::as_str)
                .unwrap_or("identity")
            {
                "identity" => Mode::Identity,
                "renumbered" => Mode::Renumbered,
                _ => return Err(Error::new("ARG_MODE", "Unsupported comparison mode")),
            };
            let period = flags
                .get("--period")
                .map(|s| {
                    let (a, b) = s
                        .split_once(':')
                        .ok_or_else(|| Error::new("ARG_PERIOD", "Expected start:end"))?;
                    Period::new(a, b)
                })
                .transpose()?;
            let mapping = flags
                .get("--mapping")
                .map(|p| read(p, 1024 * 1024).and_then(|b| strict_json::from_slice(&b)))
                .transpose()?;
            let policy = Policy {
                mode,
                period,
                mapping,
            };
            let mut report = compare::compare(&a, &b, &policy, &l)?;
            if let Some(path) = flags.get("--work-basis") {
                let basis = strict_json::from_slice(&read(path, 4 * 1024 * 1024)?)?;
                siediff::work::apply(&mut report, &a, &b, basis, &l)?;
            }
            let claim = flags
                .get("--claim")
                .map(String::as_str)
                .unwrap_or("supplied_postings");
            if ![
                "supplied_postings",
                "declared_balances",
                "native_relationships",
                "retained_history",
                "migration_readiness",
            ]
            .contains(&claim)
            {
                return Err(Error::new("ARG_CLAIM", "Unsupported selected assertion"));
            }
            let status = report.status(claim);
            let summary = serde_json::json!({"schema":"siediff.summary.v1","selected_assertion":claim,"status":status,"assertions":report.assertions.iter().map(|a|(&a.id,a.status)).collect::<Vec<_>>(),"differences_truncated":report.differences_truncated,"explanations_truncated":report.explanations_truncated,"required_work_count":report.required_work.len(),"migration_readiness":"indeterminate","accountant_acceptance":"indeterminate"});
            emit(&report, summary, &flags)?;
            Ok(status_code(status))
        }
        "controls" => {
            if positional.len() != 2 {
                return Err(Error::new(
                    "ARGUMENT",
                    "Expected ledger and control manifest",
                ));
            }
            let d = input(
                &positional[0],
                flags.get("--format"),
                encoding(flags.get("--encoding"))?,
                &l,
            )?;
            let m = strict_json::from_slice(&read(&positional[1], 4 * 1024 * 1024)?)?;
            let r = control::check(&d, &m, &l)?;
            let summary = serde_json::json!({"schema":"siediff.summary.v1","selected_assertion":"declared_independent_controls","status":r.status,"controls":r.observations.iter().map(|a|(&a.id,a.status)).collect::<Vec<_>>(),"accountant_acceptance":"indeterminate"});
            emit(&r, summary, &flags)?;
            Ok(status_code(r.status))
        }
        _ => Err(Error::new("ARG_COMMAND", "Unknown subcommand")),
    }
}
fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            let code = if e.code.starts_with("LIMIT_") {
                4
            } else if e.code.starts_with("IO_") {
                5
            } else {
                2
            };
            // Do not echo original records, paths, identifiers or OS error strings.
            eprintln!(
                "{}",
                serde_json::to_string(&e).unwrap_or_else(|_| "{\"code\":\"SERIALIZE\"}".into())
            );
            ExitCode::from(code)
        }
    }
}
