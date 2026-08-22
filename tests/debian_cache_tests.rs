use omg_lib::daemon::cache::PackageCache;
use omg_lib::daemon::protocol::PackageInfo;

#[test]
fn test_debian_search_caching() {
    let cache = PackageCache::default();
    let query = "vim".to_string();
    let results = vec![
        PackageInfo {
            name: "vim".to_string(),
            version: "1.0".to_string(),
            description: "desc".to_string(),
            source: "apt".to_string(),
        },
        PackageInfo {
            name: "vim-tiny".to_string(),
            version: "1.0".to_string(),
            description: "desc".to_string(),
            source: "apt".to_string(),
        },
    ];

    cache.insert_debian_arc(query.clone(), std::sync::Arc::new(results.clone()));

    let cached = cache.get_debian(&query);
    assert!(cached.is_some());
    let cached_pkgs = cached.unwrap();
    assert_eq!(cached_pkgs.len(), results.len());
    assert_eq!(cached_pkgs[0].name, "vim");
}
