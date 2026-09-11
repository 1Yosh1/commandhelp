// tests/repo_indexer_test.rs
use chelp::repo_indexer::{
    index_workspace, parse_docker_compose_services, parse_justfile_recipes,
    parse_makefile_targets, parse_package_json_scripts,
};
use chelp::storage::SchemaStore;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_parse_makefile() {
    let content = r#"
.PHONY: all clean

## Build the application release binary
build:
	cargo build --release

## Run test suite
test:
	cargo test

docker-build: ## Build container image
	docker build -t app:latest .
"#;

    let targets = parse_makefile_targets(content);
    assert_eq!(targets.len(), 3);
    assert_eq!(targets[0].name, "build");
    assert_eq!(
        targets[0].description.as_deref(),
        Some("Build the application release binary")
    );
    assert_eq!(targets[1].name, "test");
    assert_eq!(targets[2].name, "docker-build");
    assert_eq!(targets[2].description.as_deref(), Some("Build container image"));
}

#[test]
fn test_parse_package_json() {
    let content = r#"{
  "name": "sample-project",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "test": "vitest run"
  }
}"#;

    let scripts = parse_package_json_scripts(content);
    assert_eq!(scripts.len(), 3);
    let dev = scripts.iter().find(|s| s.name == "dev").unwrap();
    assert!(dev.description.as_ref().unwrap().contains("vite"));
}

#[test]
fn test_parse_justfile() {
    let content = r#"
# Default recipe to run tests
test:
    cargo test

# Deploy staging
deploy-staging:
    ./scripts/deploy.sh staging
"#;

    let recipes = parse_justfile_recipes(content);
    assert_eq!(recipes.len(), 2);
    assert_eq!(recipes[0].name, "test");
    assert_eq!(recipes[1].name, "deploy-staging");
}

#[test]
fn test_parse_docker_compose() {
    let content = r#"
version: '3.8'
services:
  web:
    image: nginx:alpine
    ports:
      - "80:80"
  db:
    image: postgres:15
    environment:
      POSTGRES_PASSWORD: secret
"#;

    let services = parse_docker_compose_services(content);
    assert_eq!(services, vec!["web", "db"]);
}

#[test]
fn test_index_workspace_integration() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let db_path = ws.join("test.db");
    let store = SchemaStore::new(&db_path).unwrap();

    // Create a mock Makefile and package.json
    fs::write(
        ws.join("Makefile"),
        "## Compile\ncompile:\n\tcargo check\n\n## Test\ntest:\n\tcargo test\n",
    )
    .unwrap();

    fs::write(
        ws.join("package.json"),
        r#"{"scripts": {"lint": "eslint", "format": "prettier"}}"#,
    )
    .unwrap();

    let count = index_workspace(ws, &store).unwrap();
    assert_eq!(count, 4); // 2 makefile targets + 2 npm scripts

    // Check stored make schema
    let make_schema = store.get_schema("make", &[]).unwrap().expect("Expected make schema");
    assert_eq!(make_schema.subcommands, vec!["compile", "test"]);

    // Check stored npm run schema
    let npm_schema = store
        .get_schema("npm", &["run".to_string()])
        .unwrap()
        .expect("Expected npm run schema");
    let mut npm_subs = npm_schema.subcommands;
    npm_subs.sort();
    assert_eq!(npm_subs, vec!["format", "lint"]);
}

#[test]
fn test_cli_index_repo_command() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    fs::write(
        ws.join("Makefile"),
        "## Build\nbuild:\n\techo build\n",
    )
    .unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("chelp").unwrap();
    cmd.arg("index-repo")
        .arg("--path")
        .arg(ws.to_str().unwrap())
        .env("CHELP_NO_AUTO_SPAWN", "1");

    cmd.assert()
        .success()
        .stdout(predicates::str::contains("Indexed 1 project targets/scripts into local store."));
}
