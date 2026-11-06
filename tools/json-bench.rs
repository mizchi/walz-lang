//! Native release reference. Input I/O and preflight are outside the timer.
use serde_json::{Value, json};
use std::{env, hint::black_box, time::Instant};

fn kind(value: &Value) -> u64 {
    match value {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 2,
        Value::String(_) => 3,
        Value::Array(_) => 4,
        Value::Object(_) => 5,
    }
}
fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    assert_eq!(
        args.len(),
        3,
        "json-bench parse|normalize|check input.json iterations"
    );
    let input = std::fs::read_to_string(&args[1]).unwrap();
    let count: u64 = args[2].parse().unwrap();
    assert!(count > 0 && count <= 10_000_000);
    let value: Value = serde_json::from_str(&input).unwrap();
    if args[0] == "check" {
        println!(
            "{}",
            json!({"normalized":serde_json::to_string(&value).unwrap(),"kind":kind(&value)})
        );
        return;
    }
    assert!(args[0] == "parse" || args[0] == "normalize");
    drop(value);
    // Warm the allocator on each process; construction/startup is not timed.
    for _ in 0..32 {
        black_box(serde_json::from_str::<Value>(black_box(&input)).unwrap());
    }
    let mut sink = 0_u64;
    let start = Instant::now();
    for _ in 0..count {
        let value: Value = serde_json::from_str(black_box(&input)).unwrap();
        if args[0] == "normalize" {
            let text = serde_json::to_string(black_box(&value)).unwrap();
            sink = sink.wrapping_add(text.len() as u64);
            black_box(text);
        } else {
            sink = sink.wrapping_add(kind(black_box(&value)));
        }
        // Destruction of the owned AST is included in native timings.
        black_box(value);
    }
    let elapsed = start.elapsed().as_nanos();
    println!(
        "{}",
        json!({"elapsedNs":elapsed as u64,"calls":count,"checksum":sink})
    );
}
