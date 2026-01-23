#![no_main]
use libfuzzer_sys::fuzz_target;
use omg_lib::config::Config;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = toml::from_str::<Config>(s);
    }
});
