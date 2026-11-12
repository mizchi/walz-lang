//! Offline application scaffolding; immutable template data is embedded in the CLI.
use std::{ffi::OsString, fs, io, path::Path, process::ExitCode};

pub const USAGE: &str = "Usage: walz new <directory>\n       walz new -- <directory>\n\nCreate an application with source, tests, a benchmark, bilingual references, and a local Walz skill.\nThe destination must not exist. No dependencies are downloaded and Git is not initialized.";

const FILES: &[(&str, &str)] = &[
    (
        "src/main.walz",
        include_str!("../templates/app/src/main.walz"),
    ),
    (
        "src/math.walz",
        include_str!("../templates/app/src/math.walz"),
    ),
    (
        "tests/math.test.walz",
        include_str!("../templates/app/tests/math.test.walz"),
    ),
    (
        "app_bench.walz",
        include_str!("../templates/app/app_bench.walz"),
    ),
    (".gitignore", include_str!("../templates/app/.gitignore")),
    ("README.md", include_str!("../templates/app/README.md")),
    (
        "README.ja.md",
        include_str!("../templates/app/README.ja.md"),
    ),
    ("AGENTS.md", include_str!("../templates/app/AGENTS.md")),
    ("CHEATSHEET.md", include_str!("../CHEATSHEET.md")),
    ("CHEATSHEET.ja.md", include_str!("../CHEATSHEET.ja.md")),
    (
        ".agents/skills/walz/SKILL.md",
        include_str!("../.agents/skills/walz/SKILL.md"),
    ),
    (
        ".agents/skills/walz/agents/openai.yaml",
        include_str!("../.agents/skills/walz/agents/openai.yaml"),
    ),
    (
        ".agents/skills/walz/references/syntax.md",
        include_str!("../.agents/skills/walz/references/syntax.md"),
    ),
    (
        ".agents/skills/walz/references/cli.md",
        include_str!("../.agents/skills/walz/references/cli.md"),
    ),
];

fn generate(destination: &Path) -> io::Result<()> {
    if fs::symlink_metadata(destination).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "destination already exists",
        ));
    }
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let staged = tempfile::Builder::new()
        .prefix(".walz-new-")
        .tempdir_in(parent)?;
    for (name, text) in FILES {
        let path = staged.path().join(name);
        fs::create_dir_all(path.parent().unwrap())?;
        if name.starts_with("CHEATSHEET") {
            fs::write(path, portable_cheatsheet(text))?;
        } else {
            fs::write(path, text)?;
        }
    }
    // Reserve the destination atomically. In particular, never replace an
    // existing empty directory or a symlink, even if another process won a race.
    fs::create_dir(destination)?;
    for entry in fs::read_dir(staged.path())? {
        let entry = entry?;
        fs::rename(entry.path(), destination.join(entry.file_name()))?;
    }
    Ok(())
}

/// Keep bundled language switches local and send checkout-only links to the
/// published documentation. Fenced examples retain their original bytes.
fn portable_cheatsheet(text: &str) -> String {
    let mut out = String::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.starts_with("```") {
            fenced = !fenced;
        }
        let mut rest = line;
        if !fenced && !line.starts_with("```") {
            while let Some((before, link)) = rest.split_once("](") {
                let Some((target, after)) = link.split_once(')') else {
                    break;
                };
                out.push_str(before);
                out.push_str("](");
                if !target.starts_with('#')
                    && !target.contains("://")
                    && !matches!(target, "CHEATSHEET.md" | "CHEATSHEET.ja.md")
                {
                    out.push_str("https://github.com/mizchi/walz-lang/blob/main/");
                }
                out.push_str(target);
                out.push(')');
                rest = after;
            }
        }
        out.push_str(rest);
        out.push('\n');
    }
    out
}

pub fn command(args: &[OsString]) -> ExitCode {
    let path = match args {
        [path] if !path.to_string_lossy().starts_with('-') => Path::new(path),
        [separator, path] if separator == "--" => Path::new(path),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    if path.as_os_str().is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    match generate(path) {
        Ok(()) => {
            println!(
                "Created Walz application at {}\nRun walz run, walz test, and walz bench from that directory.",
                path.display()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            if error.kind() == io::ErrorKind::AlreadyExists {
                eprintln!(
                    "cannot create {}: destination already exists",
                    path.display()
                );
            } else {
                eprintln!("cannot create {}: {error}", path.display());
            }
            ExitCode::FAILURE
        }
    }
}
