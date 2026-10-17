//! SoundTouch, built from its sources in `soundtouch/`, for its C API: only a C++ compiler is
//! needed, on the build machine and in the device's build container alike.

use std::path::Path;

fn main() {
    let root = Path::new("soundtouch");
    let library = root.join("source/SoundTouch");
    let sources: Vec<_> = std::fs::read_dir(&library)
        .expect("SoundTouch's sources")
        .map(|entry| entry.expect("a source").path())
        .filter(|path| path.extension().is_some_and(|e| e == "cpp"))
        .collect();
    cc::Build::new()
        .cpp(true)
        .files(&sources)
        .file(root.join("source/SoundTouchDLL/SoundTouchDLL.cpp"))
        .include(root.join("include"))
        .include(&library)
        // Its C API is only `extern "C"` when it is built to be exported.
        .define("DLL_EXPORTS", None)
        .warnings(false)
        .compile("soundtouch");
    println!("cargo:rerun-if-changed=soundtouch");
}
