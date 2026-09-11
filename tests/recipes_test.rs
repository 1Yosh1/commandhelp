// tests/recipes_test.rs
use chelp::models::SafetyLevel;
use chelp::recipes::{
    get_workspace_recipes_path, load_recipes, remove_recipe, save_recipe, Recipe,
};
use tempfile::tempdir;

#[test]
fn test_recipe_lifecycle_in_workspace() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    // 1. Initially empty
    let initial = load_recipes(Some(ws)).unwrap();
    // Filter out any recipes that might be in user's global config during testing
    let ws_only: Vec<_> = initial
        .into_iter()
        .filter(|r| r.name.starts_with("test-"))
        .collect();
    assert!(ws_only.is_empty());

    // 2. Save a safe recipe
    let recipe1 = Recipe {
        name: "test-build".to_string(),
        command: "cargo build --release".to_string(),
        description: "Build release artifact".to_string(),
        tags: vec!["rust".to_string(), "build".to_string()],
        safety_level: None,
    };
    let saved_path = save_recipe(recipe1.clone(), false, Some(ws)).unwrap();
    assert!(saved_path.exists());
    assert_eq!(saved_path, get_workspace_recipes_path(ws));

    // 3. Verify loaded
    let loaded = load_recipes(Some(ws)).unwrap();
    let found = loaded.iter().find(|r| r.name == "test-build").unwrap();
    assert_eq!(found.command, "cargo build --release");
    assert_eq!(found.tags, vec!["rust", "build"]);
    assert_eq!(found.resolved_safety().0, SafetyLevel::Safe);

    // 4. Save a destructive recipe and verify safety inference
    let recipe_destructive = Recipe {
        name: "test-nuke-db".to_string(),
        command: "DROP DATABASE production;".to_string(),
        description: "Nuke database".to_string(),
        tags: vec!["db".to_string()],
        safety_level: None,
    };
    save_recipe(recipe_destructive, false, Some(ws)).unwrap();

    let loaded2 = load_recipes(Some(ws)).unwrap();
    let found_destructive = loaded2.iter().find(|r| r.name == "test-nuke-db").unwrap();
    assert_eq!(found_destructive.resolved_safety().0, SafetyLevel::Destructive);

    // 5. Remove recipe
    let removed = remove_recipe("test-build", false, Some(ws)).unwrap();
    assert!(removed);

    let loaded3 = load_recipes(Some(ws)).unwrap();
    assert!(loaded3.iter().all(|r| r.name != "test-build"));
    assert!(loaded3.iter().any(|r| r.name == "test-nuke-db"));
}

#[test]
fn test_recipe_cli_commands() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    let mut add_cmd = assert_cmd::Command::cargo_bin("chelp").unwrap();
    add_cmd
        .current_dir(ws)
        .arg("recipe")
        .arg("add")
        .arg("deploy-demo")
        .arg("--cmd")
        .arg("docker compose up -d")
        .arg("--desc")
        .arg("Start services in background")
        .arg("--tag")
        .arg("deploy")
        .arg("--tag")
        .arg("docker")
        .env("CHELP_NO_AUTO_SPAWN", "1");

    add_cmd
        .assert()
        .success()
        .stdout(predicates::str::contains("Saved recipe 'deploy-demo'"));

    let mut list_cmd = assert_cmd::Command::cargo_bin("chelp").unwrap();
    list_cmd
        .current_dir(ws)
        .arg("recipe")
        .arg("list")
        .env("CHELP_NO_AUTO_SPAWN", "1");

    list_cmd
        .assert()
        .success()
        .stdout(predicates::str::contains("deploy-demo"))
        .stdout(predicates::str::contains("docker compose up -d"))
        .stdout(predicates::str::contains("Start services in background"));

    let mut remove_cmd = assert_cmd::Command::cargo_bin("chelp").unwrap();
    remove_cmd
        .current_dir(ws)
        .arg("recipe")
        .arg("remove")
        .arg("deploy-demo")
        .env("CHELP_NO_AUTO_SPAWN", "1");

    remove_cmd
        .assert()
        .success()
        .stdout(predicates::str::contains("Removed recipe 'deploy-demo'"));
}
