// src/repo_indexer.rs
use crate::error::ChelpError;
use crate::models::CliCommandSchema;
use crate::storage::SchemaStore;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredTarget {
    pub name: String,
    pub description: Option<String>,
}

/// Indexes project files in the workspace (Makefile, Justfile, package.json, docker-compose)
pub fn index_workspace(workspace_dir: &Path, store: &SchemaStore) -> Result<usize, ChelpError> {
    let mut total_indexed = 0;

    // 1. Makefile
    let makefile_path = workspace_dir.join("Makefile");
    let alt_makefile = workspace_dir.join("makefile");
    let target_makefile = if makefile_path.exists() {
        Some(makefile_path)
    } else if alt_makefile.exists() {
        Some(alt_makefile)
    } else {
        None
    };

    if let Some(mf) = target_makefile {
        if let Ok(content) = fs::read_to_string(&mf) {
            let targets = parse_makefile_targets(&content);
            if !targets.is_empty() {
                let schema = CliCommandSchema {
                    binary: "make".to_string(),
                    subcommand_path: vec![],
                    usage: "make <target>".to_string(),
                    description: "Project Makefile targets".to_string(),
                    flags: vec![],
                    subcommands: targets.iter().map(|t| t.name.clone()).collect(),
                    binary_mtime: 0,
                    last_indexed: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                };
                store.save_schema(&schema)?;
                total_indexed += targets.len();
            }
        }
    }

    // 2. package.json
    let pkg_path = workspace_dir.join("package.json");
    if pkg_path.exists() {
        if let Ok(content) = fs::read_to_string(&pkg_path) {
            let scripts = parse_package_json_scripts(&content);
            if !scripts.is_empty() {
                let schema = CliCommandSchema {
                    binary: "npm".to_string(),
                    subcommand_path: vec!["run".to_string()],
                    usage: "npm run <script>".to_string(),
                    description: "Project package.json scripts".to_string(),
                    flags: vec![],
                    subcommands: scripts.iter().map(|s| s.name.clone()).collect(),
                    binary_mtime: 0,
                    last_indexed: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                };
                store.save_schema(&schema)?;
                total_indexed += scripts.len();
            }
        }
    }

    // 3. Justfile
    let justfile_path = workspace_dir.join("Justfile");
    let alt_justfile = workspace_dir.join("justfile");
    let target_justfile = if justfile_path.exists() {
        Some(justfile_path)
    } else if alt_justfile.exists() {
        Some(alt_justfile)
    } else {
        None
    };

    if let Some(jf) = target_justfile {
        if let Ok(content) = fs::read_to_string(&jf) {
            let recipes = parse_justfile_recipes(&content);
            if !recipes.is_empty() {
                let schema = CliCommandSchema {
                    binary: "just".to_string(),
                    subcommand_path: vec![],
                    usage: "just <recipe>".to_string(),
                    description: "Project Justfile recipes".to_string(),
                    flags: vec![],
                    subcommands: recipes.iter().map(|r| r.name.clone()).collect(),
                    binary_mtime: 0,
                    last_indexed: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                };
                store.save_schema(&schema)?;
                total_indexed += recipes.len();
            }
        }
    }

    // 4. Docker Compose
    let compose_files = [
        "docker-compose.yml",
        "docker-compose.yaml",
        "compose.yml",
        "compose.yaml",
    ];
    for cf in compose_files {
        let compose_path = workspace_dir.join(cf);
        if compose_path.exists() {
            if let Ok(content) = fs::read_to_string(&compose_path) {
                let services = parse_docker_compose_services(&content);
                if !services.is_empty() {
                    let schema = CliCommandSchema {
                        binary: "docker-compose".to_string(),
                        subcommand_path: vec!["up".to_string()],
                        usage: "docker compose up <service>".to_string(),
                        description: "Docker compose services".to_string(),
                        flags: vec![],
                        subcommands: services.clone(),
                        binary_mtime: 0,
                        last_indexed: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    };
                    store.save_schema(&schema)?;
                    total_indexed += services.len();
                }
            }
            break;
        }
    }

    Ok(total_indexed)
}

/// Parses Makefile targets
pub fn parse_makefile_targets(content: &str) -> Vec<DiscoveredTarget> {
    let mut targets = Vec::new();
    let mut last_comment: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let comment_body = trimmed.trim_start_matches('#').trim().to_string();
            if !comment_body.is_empty() {
                last_comment = Some(comment_body);
            }
            continue;
        }

        // Check for target definitions: target: ...
        if let Some((target_part, _)) = trimmed.split_once(':') {
            let target_name = target_part.trim();
            // Exclude special targets (.PHONY, variables, pattern rules)
            if !target_name.starts_with('.')
                && !target_name.contains('=')
                && !target_name.contains('%')
                && !target_name.is_empty()
                && !target_name.contains(' ')
            {
                let inline_desc = line.split_once("##").map(|(_, d)| d.trim().to_string());
                let desc = inline_desc.or_else(|| last_comment.take());

                targets.push(DiscoveredTarget {
                    name: target_name.to_string(),
                    description: desc,
                });
            }
        }
        last_comment = None;
    }
    targets
}

/// Parses package.json scripts
pub fn parse_package_json_scripts(content: &str) -> Vec<DiscoveredTarget> {
    let mut scripts = Vec::new();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(map) = v.get("scripts").and_then(|s| s.as_object()) {
            for (name, val) in map {
                let cmd_str = val.as_str().unwrap_or("").to_string();
                scripts.push(DiscoveredTarget {
                    name: name.clone(),
                    description: Some(format!("Runs: {}", cmd_str)),
                });
            }
        }
    }
    scripts
}

/// Parses Justfile recipes
pub fn parse_justfile_recipes(content: &str) -> Vec<DiscoveredTarget> {
    let mut recipes = Vec::new();
    let mut last_comment: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let comment = trimmed.trim_start_matches('#').trim().to_string();
            if !comment.is_empty() {
                last_comment = Some(comment);
            }
            continue;
        }

        if let Some((recipe_part, _)) = trimmed.split_once(':') {
            let recipe_name = recipe_part.trim().split_whitespace().next().unwrap_or("");
            if !recipe_name.is_empty() && !recipe_name.starts_with('_') && !recipe_name.starts_with('@') {
                recipes.push(DiscoveredTarget {
                    name: recipe_name.to_string(),
                    description: last_comment.take(),
                });
            }
        }
        last_comment = None;
    }
    recipes
}

/// Parses Docker Compose services
pub fn parse_docker_compose_services(content: &str) -> Vec<String> {
    let mut services = Vec::new();
    let mut in_services = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "services:" {
            in_services = true;
            continue;
        }

        if in_services {
            // Service definitions are indented by 2 spaces: "  web:"
            if line.starts_with("  ") && !line.starts_with("   ") && trimmed.ends_with(':') {
                let service_name = trimmed.trim_end_matches(':').trim();
                if !service_name.is_empty() {
                    services.push(service_name.to_string());
                }
            } else if !line.starts_with(' ') && !trimmed.is_empty() && !trimmed.starts_with('#') {
                break;
            }
        }
    }
    services
}
