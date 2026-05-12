// MIT License
// Copyright (c) 2025 fi-code contributors

// =============================================================================
// Cucumber BDD 测试入口
// =============================================================================

use cucumber::World;
use fi_code_tests::bdd::AgentWorld;

#[tokio::main]
async fn main() {
    // CARGO_MANIFEST_DIR points to tests/ directory in this package
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(|s| std::path::PathBuf::from(s))
        .unwrap_or_else(|_| std::env::current_dir().unwrap());
    let features_dir = manifest_dir.join("bdd/features");
    
    AgentWorld::cucumber()
        .max_concurrent_scenarios(1)
        .run_and_exit(features_dir)
        .await;
}
