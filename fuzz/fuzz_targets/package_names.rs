#![no_main]

use libfuzzer_sys::fuzz_target;
use omg_lib::core::security::validate_package_name;

fuzz_target!(|data: &[u8]| {
    let Ok(name) = std::str::from_utf8(data) else {
        return;
    };

    let first_result = validate_package_name(name);
    let second_result = validate_package_name(name);
    assert_eq!(first_result.is_ok(), second_result.is_ok());

    let has_forbidden_character = name
        .chars()
        .any(|character| !character.is_ascii_alphanumeric() && !"-_+.@/".contains(character));
    let must_be_rejected = name.is_empty()
        || name.len() > 255
        || name.starts_with('-')
        || name.starts_with('.')
        || name.starts_with('/')
        || name.contains("..")
        || has_forbidden_character;

    if must_be_rejected {
        assert!(
            first_result.is_err(),
            "unsafe package name was accepted: {name:?}"
        );
    }
});
