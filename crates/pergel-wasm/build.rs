fn main() {
    println!("cargo:rerun-if-changed=c/gl.c");
    println!("cargo:rerun-if-changed=c/sjlj.c");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("emscripten") {
        cc::Build::new().file("c/gl.c").file("c/sjlj.c").compile("pergel_gl");
    }
}
