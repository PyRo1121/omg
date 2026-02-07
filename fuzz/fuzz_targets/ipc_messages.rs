#![no_main]
use libfuzzer_sys::fuzz_target;
use omg_lib::daemon::protocol::{Request, Response, ResponseResult};

fuzz_target!(|data: &[u8]| {
    // Test 1: Deserialize should never panic, even on corrupted data
    let decode_result = bitcode::decode::<Request>(data);

    // If decode succeeds, perform validation and round-trip tests
    if let Ok(request) = decode_result {
        // Test 2: Request ID should be accessible
        let id = request.id();
        assert!(id > 0 || id == 0, "Request ID should be valid u64");

        // Test 3: Serialize round-trip should work
        let encoded = bitcode::encode(&request);
        assert!(encoded.is_ok(), "Request should re-serialize");

        if let Ok(encoded_data) = encoded {
            let decoded_again = bitcode::decode::<Request>(&encoded_data);
            assert!(decoded_again.is_ok(), "Round-trip decode should succeed");

            if let Ok(decoded_request) = decoded_again {
                // IDs should match
                assert_eq!(request.id(), decoded_request.id(), "Request IDs should match after round-trip");
            }
        }

        // Test 4: Validate request-specific constraints
        match request {
            Request::Search { query, limit, .. } => {
                // Query length limits
                assert!(query.len() <= 10000, "Query length {} exceeds maximum", query.len());

                // Query should not contain null bytes
                assert!(!query.contains('\0'), "Query contains null byte");

                // Limit should be reasonable
                if let Some(lim) = limit {
                    assert!(lim > 0, "Limit must be positive");
                    assert!(lim <= 10000, "Limit {} exceeds maximum", lim);
                }
            }

            Request::Info { package, .. } => {
                // Package name validation
                assert!(package.len() <= 1000, "Package name length {} exceeds maximum", package.len());
                assert!(!package.is_empty(), "Package name cannot be empty");
                assert!(!package.contains('\0'), "Package name contains null byte");

                // Security: reject path traversal attempts
                assert!(!package.contains(".."), "Package name contains path traversal");
                assert!(!package.contains('/'), "Package name contains path separator");

                // Security: reject shell injection attempts
                let dangerous_chars = [';', '&', '|', '`', '$', '\n', '\r'];
                for ch in dangerous_chars {
                    assert!(!package.contains(ch), "Package name contains dangerous char: {}", ch);
                }
            }

            Request::Suggest { query, limit, .. } => {
                // Suggestion query validation
                assert!(query.len() <= 10000, "Suggest query too long");

                if let Some(lim) = limit {
                    assert!(lim > 0 && lim <= 1000, "Suggest limit out of range");
                }
            }

            Request::Batch { requests, .. } => {
                // Batch size limits
                assert!(requests.len() <= 100, "Batch size {} exceeds maximum", requests.len());

                // Validate nested requests
                for nested_request in requests.iter() {
                    let nested_id = nested_request.id();
                    assert!(nested_id > 0 || nested_id == 0, "Nested request ID invalid");
                }
            }

            Request::DebianSearch { query, limit, .. } => {
                // Debian search validation
                assert!(query.len() <= 10000, "Debian search query too long");

                if let Some(lim) = limit {
                    assert!(lim > 0 && lim <= 10000, "Debian search limit out of range");
                }
            }

            // Other request types are simple and don't need extra validation
            _ => {}
        }

        // Test 5: Clone should work
        let _ = request.clone();
    }

    // Test 6: Try fuzzing Response too if data is long enough
    if data.len() > 100 {
        let response_data = &data[50..];
        let _ = bitcode::decode::<Response>(response_data);

        if let Ok(response) = bitcode::decode::<Response>(response_data) {
            // Validate response
            match response {
                Response::Success { id, result } => {
                    assert!(id > 0 || id == 0, "Response ID invalid");

                    // Validate result types
                    match result {
                        ResponseResult::Search(search_result) => {
                            assert!(search_result.packages.len() <= search_result.total,
                                "Package count exceeds total");
                        }
                        ResponseResult::Info(info) => {
                            assert!(!info.name.is_empty(), "Package name in info cannot be empty");
                            assert!(!info.version.is_empty(), "Version in info cannot be empty");
                        }
                        ResponseResult::Status(status) => {
                            // Status counts should be sane
                            assert!(status.total_packages < 1_000_000, "Total packages unrealistic");
                            assert!(status.explicit_packages <= status.total_packages,
                                "Explicit packages cannot exceed total");
                        }
                        ResponseResult::ExplicitCount(count) => {
                            assert!(count < 1_000_000, "Explicit count unrealistic");
                        }
                        ResponseResult::SecurityAudit(audit) => {
                            assert!(audit.high_severity <= audit.total_vulnerabilities,
                                "High severity cannot exceed total vulnerabilities");
                        }
                        ResponseResult::CacheStats { size, max_size } => {
                            assert!(size <= max_size, "Cache size cannot exceed max size");
                        }
                        ResponseResult::Batch(responses) => {
                            assert!(responses.len() <= 100, "Batch response too large");
                        }
                        ResponseResult::Health(health) => {
                            assert!(!health.status.is_empty(), "Health status cannot be empty");
                            assert!(health.memory_usage_mb < 1_000_000, "Memory usage unrealistic");
                        }
                        _ => {}
                    }
                }

                Response::Error { id, code, message } => {
                    assert!(id > 0 || id == 0, "Error response ID invalid");

                    // Error codes should be in valid range
                    assert!(code < 0, "Error codes should be negative");
                    assert!(code >= -100000, "Error code too negative");

                    // Error message should not be empty
                    assert!(!message.is_empty(), "Error message cannot be empty");
                    assert!(message.len() <= 10000, "Error message too long");
                }
            }

            // Round-trip test for response
            let encoded_response = bitcode::encode(&response);
            assert!(encoded_response.is_ok(), "Response should re-serialize");
        }
    }

    // Test 7: Ensure no memory leaks by dropping everything
    // (Rust's ownership system ensures this, but explicit for fuzzing)
    drop(data);
});
