#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn test_cache_basic_ops() {
    let cache = PackageCache::new(10, 60);
    let pkg = PackageInfo {
        name: "test".to_string(),
        version: "1.0".to_string(),
        description: "desc".to_string(),
        source: crate::daemon::protocol::WirePackageSource::Official,
    };

    // Insert
    cache.insert_arc("query".to_string(), Arc::new(vec![pkg]));

    // Get
    let res = cache.get("query").unwrap();
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].name, "test");

    // Flush Moka's pending accounting before asserting the public statistic.
    cache.sync();
    assert_eq!(cache.stats().size, 1);

    // Clear
    cache.clear();
    assert!(cache.get("query").is_none());
}

#[test]
fn test_cache_miss_handling() {
    let cache = PackageCache::new(10, 60);

    assert!(!cache.is_info_miss("missing"));
    cache.insert_info_miss("missing");
    assert!(cache.is_info_miss("missing"));

    // Inserting info should clear miss
    let info = DetailedPackageInfo {
        name: "missing".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        url: String::new(),
        size: 0,
        download_size: 0,
        repo: String::new(),
        depends: vec![],
        licenses: vec![],
        source: crate::daemon::protocol::WirePackageSource::Official,
    };
    cache.insert_info(info);
    assert!(!cache.is_info_miss("missing"));
}

#[test]
fn oversized_search_value_is_not_admitted_to_the_byte_bounded_cache() {
    let cache = PackageCache::new(1, 60);
    let oversized = PackageInfo {
        name: "oversized".to_string(),
        version: "1".to_string(),
        description: "x".repeat(CACHE_BYTES_PER_CONFIGURED_ENTRY * 2),
        source: crate::daemon::protocol::WirePackageSource::Official,
    };

    cache.insert_arc("large".to_string(), Arc::new(vec![oversized]));
    cache.sync();

    assert!(cache.get("large").is_none());
}

#[test]
fn status_refresh_invalidates_the_previous_explicit_list() {
    let cache = PackageCache::new_with_ttls(10, 600, 60);
    cache.update_explicit(vec!["old-package".to_string()]);
    assert_eq!(cache.get_explicit().unwrap().as_slice(), ["old-package"]);

    cache.update_status(Arc::new(StatusResult {
        total_packages: 100,
        explicit_packages: 2,
        orphan_packages: 5,
        updates_available: 2,
        security_vulnerabilities: 0,
        vulnerabilities_scanned: true,
        runtime_versions: vec![],
    }));

    cache.sync();
    assert_eq!(cache.get_explicit_count(), Some(2));
    assert!(cache.get_explicit().is_none());
}

#[test]
fn test_system_status_cache() {
    let cache = PackageCache::new(10, 60);
    let status = StatusResult {
        total_packages: 100,
        explicit_packages: 10,
        orphan_packages: 5,
        updates_available: 2,
        security_vulnerabilities: 0,
        vulnerabilities_scanned: true,
        runtime_versions: vec![],
    };

    cache.update_status(Arc::new(status));
    let cached = cache.get_status().unwrap();
    assert_eq!(cached.total_packages, 100);
}
