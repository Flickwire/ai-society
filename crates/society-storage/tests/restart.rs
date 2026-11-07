//! Persistence across a closed connection and a reopened process boundary.
use society_sim::Village;
use society_storage::Store;

#[tokio::test]
async fn latest_checkpoint_survives_reopening_and_can_resume() {
    let path = std::env::temp_dir().join(format!(
        "ai-society-restart-{}-{}.sqlite",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let mut village = Village::new();
    let store = Store::open(&path).await.unwrap();
    assert!(store.load().await.unwrap().is_none());
    store.save(&village).await.unwrap();
    village.run_for(12).unwrap();
    store.save(&village).await.unwrap();
    drop(store);
    let store = Store::open(&path).await.unwrap();
    let mut resumed = store.load().await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(village.snapshot()).unwrap(),
        serde_json::to_value(resumed.snapshot()).unwrap()
    );
    village.run_for(12).unwrap();
    resumed.run_for(12).unwrap();
    assert_eq!(
        serde_json::to_value(village.snapshot()).unwrap(),
        serde_json::to_value(resumed.snapshot()).unwrap()
    );
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn committed_checkpoint_survives_abrupt_process_exit() {
    const PATH_VAR: &str = "AI_SOCIETY_CRASH_TEST_PATH";
    if let Some(path) = std::env::var_os(PATH_VAR) {
        let store = Store::open(path).await.unwrap();
        let mut village = Village::new();
        village.run_for(3).unwrap();
        store.save(&village).await.unwrap();
        std::process::exit(0);
    }
    let path = std::env::temp_dir().join(format!("ai-society-crash-{}.sqlite", std::process::id()));
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "committed_checkpoint_survives_abrupt_process_exit",
        ])
        .env(PATH_VAR, &path)
        .status()
        .unwrap();
    assert!(status.success());
    let store = Store::open(&path).await.unwrap();
    assert_eq!(store.load().await.unwrap().unwrap().now().0, 3);
    drop(store);
    std::fs::remove_file(path).unwrap();
}
