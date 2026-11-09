fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // RVZ intake overflows MSVC's default 1 MiB main stack in debug builds.
        // Reserve address space; Windows commits pages as needed. Keep winit on
        // the original main thread rather than moving all commands to a worker.
        println!("cargo::rustc-link-arg-bin=goldeneye=/STACK:8388608");
    }
}
