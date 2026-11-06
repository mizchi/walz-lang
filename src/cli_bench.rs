use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use wlt::benchmark::{BenchOptions, BenchOutcome};

fn arguments(args: &[OsString]) -> Result<(Vec<PathBuf>, BenchOptions, bool), String> {
    let mut paths = Vec::new();
    let mut options = BenchOptions::default();
    let mut json = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--list") => options.list = true,
            Some("--json") => json = true,
            Some("--filter") => {
                options.filter = Some(
                    args.next()
                        .and_then(|s| s.to_str())
                        .ok_or("--filter requires UTF-8 text")?
                        .to_owned(),
                )
            }
            Some(
                flag @ ("--samples" | "--iterations" | "--warmup-ms" | "--sample-ms"
                | "--timeout-ms"),
            ) => {
                let value = args
                    .next()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.parse::<u64>().ok())
                    .ok_or_else(|| format!("{flag} requires a nonnegative integer"))?;
                match flag {
                    "--samples" => {
                        options.samples = u32::try_from(value).map_err(|e| e.to_string())?
                    }
                    "--iterations" => {
                        options.iterations = Some(u32::try_from(value).map_err(|e| e.to_string())?)
                    }
                    "--warmup-ms" => options.warmup_ms = value,
                    "--sample-ms" => options.sample_ms = value,
                    "--timeout-ms" => options.timeout_ms = value,
                    _ => unreachable!(),
                }
            }
            Some("--") => {
                paths.extend(args.map(PathBuf::from));
                break;
            }
            Some(flag) if flag.starts_with('-') => {
                return Err(format!("unknown bench option {flag:?}"));
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    options.validate()?;
    Ok((paths, options, json))
}

pub fn command(args: &[OsString]) -> ExitCode {
    let (paths, options, json) = match arguments(args) {
        Ok(arguments) => arguments,
        Err(error) => {
            eprintln!("{error}\n{}", super::USAGE);
            return ExitCode::from(2);
        }
    };
    let report = wlt::benchmark::run(&paths, &options);
    if json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        for bench in &report.benchmarks {
            let path = bench.path.display();
            match &bench.outcome {
                BenchOutcome::Listed => println!("{path}::{:?}: bench", bench.name),
                BenchOutcome::Measured { summary } => println!(
                    "bench {path}::{:?} ... {:.2} ns/op (median); mean {:.2}; min {:.2}; max {:.2}; stddev {:.2}; {} samples × {} iterations",
                    bench.name,
                    summary.median_ns,
                    summary.mean_ns,
                    summary.min_ns,
                    summary.max_ns,
                    summary.std_dev_ns,
                    summary.samples_ns.len(),
                    summary.iterations,
                ),
                BenchOutcome::Failed { message } => {
                    println!("bench {path}::{:?} ... FAILED\n  {message}", bench.name)
                }
                BenchOutcome::TimedOut => println!(
                    "bench {path}::{:?} ... FAILED\n  timed out after {} ms",
                    bench.name, options.timeout_ms
                ),
            }
        }
        for error in &report.errors {
            let path = error
                .path
                .as_ref()
                .map(|p| format!("{}: ", p.display()))
                .unwrap_or_default();
            println!("ERROR {path}{}", error.message);
        }
        if options.list {
            println!(
                "{} benchmarks; {} filtered out; {} errors",
                report.benchmarks.len(),
                report.filtered_out,
                report.errors.len()
            );
        } else {
            println!(
                "{} measured; {} failed; {} errors; {} filtered out",
                report.measured(),
                report.failed(),
                report.errors.len(),
                report.filtered_out
            );
        }
    }
    if report.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
