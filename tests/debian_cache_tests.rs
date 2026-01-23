use omg_lib::daemon::cache::PackageCache;

#[test]
fn test_debian_search_caching() {
    let cache = PackageCache::default();
    let query = "vim".to_string();
    let results = vec!["vim".to_string(), "vim-tiny".to_string()];

    // These methods don't exist yet, so this will fail to compile
    cache.insert_debian(query.clone(), results.clone());
    
    let cached = cache.get_debian(&query);
    assert!(cached.is_some());
    assert_eq!(cached.unwrap(), results);
}
