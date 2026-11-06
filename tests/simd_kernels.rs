#[test]
fn jsimd_style_kernels_stay_inside_the_checked_core() {
    for path in [
        "examples/simd-kernels/resident.wlt",
        "examples/simd-kernels/portable.wlt",
    ] {
        let source = wlt::source::load(std::path::Path::new(path)).unwrap();
        let compiled = wlt::compile(source.text()).unwrap();
        assert!(!compiled.analysis.opaque, "{path}");
        assert!(compiled.ir.is_some(), "{path}");
        assert!(compiled.wat.contains("i64x2.extend_low_i32x4_s"));
        assert!(compiled.wat.contains("i8x16.popcnt"));
        assert!(compiled.wat.contains("f32x4.mul"));
        assert!(compiled.wat.contains("i8x16.bitmask"));
    }
}
