use omg_lib::daemon::cache::PackageCache;
use omg_lib::daemon::protocol::PackageInfo;

fn pkg(name: &str, version: &str) -> PackageInfo {
    PackageInfo {
        name: name.to_string(),
        version: version.to_string(),
        description: "desc".to_string(),
        source: "apt".to_string(),
    }
}

#[test]
fn test_debian_search_caching() {
    let cache = PackageCache::default();
    let query = "vim".to_string();
    let results = vec![pkg("vim", "1.0"), pkg("vim-tiny", "1.0")];

    // Miss before insert: the getter must not fabricate entries.
    assert!(
        cache.get_debian(&query).is_none(),
        "unknown query must be a cache miss"
    );

    cache.insert_debian_arc(query.clone(), std::sync::Arc::new(results.clone()));

    let cached = cache
        .get_debian(&query)
        .expect("query must hit right after insert");
    assert_eq!(cached.len(), results.len());
    assert_eq!(cached[0].name, "vim");
    assert_eq!(cached[0].version, "1.0");
    assert_eq!(cached[1].name, "vim-tiny");

    // A different query must not collide with the stored one.
    assert!(
        cache.get_debian("vi").is_none(),
        "unrelated query must remain a miss"
    );
}

#[test]
fn test_debian_search_cache_overwrites_same_query() {
    let cache = PackageCache::default();
    let query = "htop".to_string();

    cache.insert_debian_arc(
        query.clone(),
        std::sync::Arc::new(vec![pkg("htop", "3.2.1")]),
    );
    let updated = vec![pkg("htop", "3.3.0"), pkg("htop-dev", "3.3.0")];
    cache.insert_debian_arc(query.clone(), std::sync::Arc::new(updated.clone()));

    let cached = cache
        .get_debian(&query)
        .expect("re-inserted query must still hit");
    assert_eq!(
        cached.len(),
        updated.len(),
        "re-insert must replace the previous entry, not append"
    );
    assert_eq!(
        cached[0].version, "3.3.0",
        "retrieval must return the newest value"
    );
}
