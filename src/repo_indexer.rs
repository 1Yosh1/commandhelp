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
#[allow(clippy::too_many_arguments)]
fn index_runner_file<F>(
    workspace_dir: &Path,
    store: &SchemaStore,
    filenames: &[&str],
    binary: &str,
    subcommand_path: Vec<String>,
    usage: &str,
    description: &str,
    parse_fn: F,
) -> Result<usize, ChelpError>
where
    F: Fn(&str) -> Vec<String>,
{
    for fname in filenames {
        let file_path = workspace_dir.join(fname);
        if file_path.exists() {
            if let Ok(content) = fs::read_to_string(&file_path) {
                let subcommands = parse_fn(&content);
                if !subcommands.is_empty() {
                    let schema = CliCommandSchema {
                        binary: binary.to_string(),
                        subcommand_path,
                        usage: usage.to_string(),
                        description: description.to_string(),
                        flags: vec![],
                        subcommands: subcommands.clone(),
                        binary_mtime: 0,
                        last_indexed: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    };
                    store.save_schema(&schema)?;
                    return Ok(subcommands.len());
                }
            }
            break;
        }
    }
    Ok(0)
}

pub fn index_workspace(workspace_dir: &Path, store: &SchemaStore) -> Result<usize, ChelpError> {
    let mut total_indexed = 0;

    // 1. Makefile
    total_indexed += index_runner_file(
        workspace_dir,
        store,
        &["Makefile", "makefile"],
        "make",
        vec![],
        "make <target>",
        "Project Makefile targets",
        |content| {
            parse_makefile_targets(content)
                .into_iter()
                .map(|t| t.name)
                .collect()
        },
    )?;

    // 2. package.json
    total_indexed += index_runner_file(
        workspace_dir,
        store,
        &["package.json"],
        "npm",
        vec!["run".to_string()],
        "npm run <script>",
        "Project package.json scripts",
        |content| {
            parse_package_json_scripts(content)
                .into_iter()
                .map(|t| t.name)
                .collect()
        },
    )?;

    // 3. Justfile
    total_indexed += index_runner_file(
        workspace_dir,
        store,
        &["Justfile", "justfile"],
        "just",
        vec![],
        "just <recipe>",
        "Project Justfile recipes",
        |content| {
            parse_justfile_recipes(content)
                .into_iter()
                .map(|t| t.name)
                .collect()
        },
    )?;

    // 4. Docker Compose
    total_indexed += index_runner_file(
        workspace_dir,
        store,
        &[
            "docker-compose.yml",
            "docker-compose.yaml",
            "compose.yml",
            "compose.yaml",
        ],
        "docker-compose",
        vec!["up".to_string()],
        "docker compose up <service>",
        "Docker compose services",
        parse_docker_compose_services,
    )?;

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
            let recipe_name = recipe_part.split_whitespace().next().unwrap_or("");
            if !recipe_name.is_empty()
                && !recipe_name.starts_with('_')
                && !recipe_name.starts_with('@')
            {
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
