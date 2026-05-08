#[cfg(feature = "cpp")]
#[cfg(not(target_os = "macos"))]
fn main() {
    cpp_build()
        .static_crt(false)
        .file("src/esaxx.cpp")
        .include("src")
        .compile("esaxx");
}

#[cfg(feature = "cpp")]
#[cfg(target_os = "macos")]
fn main() {
    let mut build = cpp_build();
    build
        .flag("-stdlib=libc++")
        .static_crt(false)
        .file("src/esaxx.cpp")
        .include("src")
        .compile("esaxx");
}

#[cfg(feature = "cpp")]
fn cpp_build() -> cc::Build {
    let mut build = cc::Build::new();
    build.cpp(true);
    if std::env::var("TARGET")
        .map(|target| target.contains("msvc"))
        .unwrap_or(false)
    {
        build.flag_if_supported("/std:c++14");
    } else {
        build.flag("-std=c++11");
    }
    build
}

#[cfg(not(feature = "cpp"))]
fn main() {}
