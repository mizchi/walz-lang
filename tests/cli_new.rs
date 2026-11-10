use std::path::Path;
use std::process::{Command, Output};

fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_walz"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn success(output: Output) -> Output {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn new_generates_an_offline_project_with_working_build_run_test_and_bench() {
    let root = tempfile::tempdir().unwrap();
    // Generation must work from the installed binary without Cargo, Git, Node,
    // a checkout, or an internet connection.
    success(
        Command::new(env!("CARGO_BIN_EXE_walz"))
            .current_dir(root.path())
            .env("PATH", "")
            .args(["new", "nested/example app"])
            .output()
            .unwrap(),
    );
    let app = root.path().join("nested/example app");
    for path in [
        "src/main.walz",
        "src/math.walz",
        "tests/math.test.walz",
        "app_bench.walz",
        "README.md",
        "README.ja.md",
        "CHEATSHEET.md",
        "CHEATSHEET.ja.md",
        "AGENTS.md",
        ".agents/skills/walz/SKILL.md",
        ".agents/skills/walz/references/syntax.md",
        ".agents/skills/walz/references/cli.md",
    ] {
        assert!(app.join(path).is_file(), "missing {path}");
    }
    let cheat = std::fs::read_to_string(app.join("CHEATSHEET.md")).unwrap();
    assert!(cheat.contains("https://github.com/mizchi/walz-lang/blob/main/"));
    assert!(
        !cheat.contains("](docs/"),
        "installed cheat sheet must not link to absent checkout files"
    );
    assert!(!cheat.contains("cargo run --locked"));
    assert!(!cheat.contains("node runtime/"));
    success(cli(&app, &["check", "src/main.walz"]));
    success(cli(&app, &["fmt-check", "src/main.walz"]));
    success(cli(&app, &["fmt-check", "src/math.walz"]));
    success(cli(&app, &["build"]));
    assert!(app.join("target/main.wasm").is_file());
    let run = success(cli(&app, &["run"]));
    assert_eq!(String::from_utf8(run.stdout).unwrap().trim(), "42");
    success(cli(&app, &["test"]));
    success(cli(&app, &["bench", "--list"]));
    success(cli(
        &app,
        &[
            "bench",
            "--samples",
            "1",
            "--iterations",
            "2",
            "--warmup-ms",
            "0",
        ],
    ));
    success(cli(&app, &["run", "--target", "core-js"]));
}

#[test]
fn new_refuses_existing_files_directories_and_symlinks_without_modification() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file"), "keep").unwrap();
    std::fs::create_dir(root.path().join("empty")).unwrap();
    std::fs::create_dir(root.path().join("occupied")).unwrap();
    std::fs::write(root.path().join("occupied/user.txt"), "keep").unwrap();
    for path in ["file", "empty", "occupied", "."] {
        let output = cli(root.path(), &["new", path]);
        assert_eq!(output.status.code(), Some(1), "{path}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    }
    assert_eq!(
        std::fs::read_to_string(root.path().join("file")).unwrap(),
        "keep"
    );
    assert_eq!(
        std::fs::read_dir(root.path().join("empty"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        std::fs::read_dir(root.path().join("occupied"))
            .unwrap()
            .count(),
        1
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.path().join("missing"), root.path().join("link")).unwrap();
        assert_eq!(cli(root.path(), &["new", "link"]).status.code(), Some(1));
        assert!(!root.path().join("missing").exists());
    }
}

#[test]
fn new_handles_help_options_and_literal_paths() {
    let root = tempfile::tempdir().unwrap();
    for args in [
        &["new"][..],
        &["new", "--unknown"],
        &["new", "one", "two"],
        &["new", "--"],
        &["new", ""],
    ] {
        assert_eq!(cli(root.path(), args).status.code(), Some(2), "{args:?}");
    }
    for args in [&["new", "--help"][..], &["help", "new"]] {
        let output = success(cli(root.path(), args));
        assert!(String::from_utf8_lossy(&output.stdout).contains("walz new"));
    }
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    success(cli(root.path(), &["new", "--", "-app"]));
    assert!(root.path().join("-app/src/main.walz").is_file());
}
