#![no_main]

use libfuzzer_sys::fuzz_target;
use omg_lib::config::Settings;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(settings) = toml::from_str::<Settings>(source) else {
        return;
    };

    let serialized = toml::to_string(&settings)
        .expect("invariant: successfully parsed settings must be serializable");
    let reparsed = toml::from_str::<Settings>(&serialized)
        .expect("invariant: serialized settings must be parseable");
    let reserialized = toml::to_string(&reparsed)
        .expect("invariant: round-tripped settings must remain serializable");

    assert_eq!(serialized, reserialized);
});
