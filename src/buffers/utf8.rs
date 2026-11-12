//! A single validator algorithm for whole byte storage and explicit ranges.
//! No byte outside the selected range participates in validation or lookahead.
pub(super) fn helpers(ranged: bool) -> String {
    let (name, params, init, end) = if ranged {
        (
            "$__utf8_range",
            " (param $start i32) (param $n i32)",
            r#"
        (if (i32.gt_u (local.get $start) (array.len (local.get $a))) (then unreachable))
        (if (i32.gt_u (local.get $n) (i32.sub (array.len (local.get $a)) (local.get $start))) (then unreachable))
        (local.set $i (local.get $start))
        (local.set $end (i32.add (local.get $start) (local.get $n)))"#,
            "(local.get $end)",
        )
    } else {
        ("$__utf8", "", "", "(array.len (local.get $a))")
    };
    let locals = if ranged { " (local $end i32)" } else { "" };
    // Entered only at a character boundary with an ASCII first byte. Stop
    // before the first non-ASCII byte; all eight indices are within the range.
    let ascii = (1..8).map(|offset| format!(
        "(if (i32.ge_u (array.get_u $__byte_storage (local.get $a) (i32.add (local.get $i) (i32.const {offset}))) (i32.const 128)) (then (local.set $i (i32.add (local.get $i) (i32.const {offset}))) (br $next)))"
    )).collect::<String>();
    let mut out = format!(
        r#"      (func {name} (param $a (ref $__byte_storage)){params} (result i32)
        (local $i i32) (local $b i32) (local $remaining i32) (local $lo i32) (local $hi i32){locals}{init}
        (local.set $lo (i32.const 128)) (local.set $hi (i32.const 191))
        (loop $next
          (if (i32.lt_u (local.get $i) {end}) (then
            (local.set $b (array.get_u $__byte_storage (local.get $a) (local.get $i)))
            (if (local.get $remaining) (then
              (if (i32.or (i32.lt_u (local.get $b) (local.get $lo)) (i32.gt_u (local.get $b) (local.get $hi))) (then (return (i32.const 0))))
              (local.set $remaining (i32.sub (local.get $remaining) (i32.const 1)))
              (local.set $lo (i32.const 128)) (local.set $hi (i32.const 191)))
            (else
              (if (i32.ge_u (local.get $b) (i32.const 128)) (then
                (if (i32.or (i32.lt_u (local.get $b) (i32.const 194)) (i32.gt_u (local.get $b) (i32.const 244))) (then (return (i32.const 0))))
                (local.set $remaining (i32.const 1))
                (if (i32.ge_u (local.get $b) (i32.const 224)) (then (local.set $remaining (i32.const 2))))
                (if (i32.ge_u (local.get $b) (i32.const 240)) (then (local.set $remaining (i32.const 3))))
                (if (i32.eq (local.get $b) (i32.const 224)) (then (local.set $lo (i32.const 160))))
                (if (i32.eq (local.get $b) (i32.const 237)) (then (local.set $hi (i32.const 159))))
                (if (i32.eq (local.get $b) (i32.const 240)) (then (local.set $lo (i32.const 144))))
                (if (i32.eq (local.get $b) (i32.const 244)) (then (local.set $hi (i32.const 143)))))
              (else
                (if (i32.ge_u (i32.sub {end} (local.get $i)) (i32.const 8)) (then
                  {ascii}
                  (local.set $i (i32.add (local.get $i) (i32.const 8))) (br $next)))))))
            (local.set $i (i32.add (local.get $i) (i32.const 1))) (br $next))))
        (i32.eqz (local.get $remaining)))"#
    );
    if ranged {
        // Existing whole-value decode paths share the range algorithm.
        out.push_str("(func $__utf8 (param $a (ref $__byte_storage)) (result i32) (call $__utf8_range (local.get $a) (i32.const 0) (array.len (local.get $a))))");
    }
    out
}
