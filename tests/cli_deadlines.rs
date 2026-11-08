#[test]
fn run_deadline_reports_timeout_after_host_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.wlt"),r#"(module (import "wlt:host/event-loop@0.1.0" "sleep" (func $sleep (param $n i32))) (func $main (export "main") (%effect host suspends) (call $sleep 60000)))"#).unwrap();
    std::fs::write(dir.path().join("host.mjs"),r#"import {writeFileSync} from 'node:fs';export default {eventLoop:{sleep:()=>{const done=Promise.withResolvers();done.promise[Symbol.asyncDispose]=async()=>{await new Promise(r=>setTimeout(r,10));writeFileSync(new URL('./closed',import.meta.url),'yes');done.resolve();};return done.promise;}}};"#).unwrap();
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_wlt"))
            .current_dir(dir.path())
            .args(args)
            .output()
            .unwrap()
    };
    let out = run(&[
        "run",
        "--target",
        "core-js",
        "--timeout-ms",
        "50",
        "--host",
        "host.mjs",
    ]);
    assert_eq!(
        out.status.code(),
        Some(124),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("closed")).unwrap(),
        "yes"
    );
    for args in [
        vec!["run", "--timeout-ms", "50"],
        vec!["run", "--target", "core-js", "--timeout-ms", "2147483648"],
    ] {
        assert_eq!(run(&args).status.code(), Some(2));
    }
}
