use omg_lib::cli::packages;

#[tokio::test]
async fn test_search_compilation() {
    let _ = packages::search("query", false, false, false).await;
}

#[tokio::test]
async fn test_install_compilation() {
    let _ = packages::install(&["package".to_string()], true, false).await;
}
