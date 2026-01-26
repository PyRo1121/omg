use omg_lib::config::Settings;
use omg_lib::core::env::distro::detect_distro;
use std::path::Path;
use std::time::Instant;

fn main() {
    println!("OMG Performance Audit");
    println!("=====================");

    let start = Instant::now();
    let settings = Settings::load();
    let duration = start.elapsed();
    println!("Settings::load(): {duration:?} (ok: {})", settings.is_ok());

    let start = Instant::now();
    let distro = detect_distro();
    let duration = start.elapsed();
    println!("detect_distro(): {duration:?} (distro: {distro:?})");

    let start = Instant::now();
    let versions = omg_lib::hooks::detect_versions(Path::new("."));
    let duration = start.elapsed();
    println!(
        "hooks::detect_versions(\".\"): {duration:?} (found: {})",
        versions.len()
    );

    let start = Instant::now();
    let _args: Vec<String> = std::env::args().collect();
    let duration = start.elapsed();
    println!("Arg collection: {duration:?}");
}
