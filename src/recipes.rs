// src/recipes.rs
use crate::config::get_config_dir;
use crate::error::ChelpError;
use crate::models::{AiCommandResponse, SafetyLevel};
use crate::safety::evaluate_command_safety;
use crate::tui::{render_interactive_confirmation, UserAction};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Recipe {
    pub name: String,
    pub command: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub safety_level: Option<SafetyLevel>,
}

impl Recipe {
    pub fn resolved_safety(&self) -> (SafetyLevel, Option<String>) {
        if let Some(level) = self.safety_level {
            (level, None)
        } else {
            evaluate_command_safety(&self.command)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecipeBook {
    #[serde(default)]
    pub recipe: Vec<Recipe>,
}

/// Returns the path to the workspace recipes file (.chelp/recipes.toml or recipes.toml)
pub fn get_workspace_recipes_path(workspace_dir: &Path) -> PathBuf {
    let dot_chelp = workspace_dir.join(".chelp").join("recipes.toml");
    if dot_chelp.exists() {
        return dot_chelp;
    }
    let flat_recipes = workspace_dir.join("recipes.toml");
    if flat_recipes.exists() {
        return flat_recipes;
    }
    // Default to .chelp/recipes.toml if neither exists yet
    dot_chelp
}

/// Returns the path to the global user recipes file (~/.config/chelp/recipes.toml)
pub fn get_global_recipes_path() -> PathBuf {
    get_config_dir().join("recipes.toml")
}

/// Loads all recipes: merges global recipes with workspace recipes (workspace takes precedence on name collisions)
pub fn load_recipes(workspace_dir: Option<&Path>) -> Result<Vec<Recipe>, ChelpError> {
    let mut map = HashMap::new();

    // 1. Load global
    let global_path = get_global_recipes_path();
    if global_path.exists() {
        if let Ok(content) = fs::read_to_string(&global_path) {
            if let Ok(book) = toml::from_str::<RecipeBook>(&content) {
                for r in book.recipe {
                    map.insert(r.name.clone(), r);
                }
            }
        }
    }

    // 2. Load workspace
    if let Some(ws) = workspace_dir {
        let ws_path = get_workspace_recipes_path(ws);
        if ws_path.exists() {
            if let Ok(content) = fs::read_to_string(&ws_path) {
                if let Ok(book) = toml::from_str::<RecipeBook>(&content) {
                    for r in book.recipe {
                        map.insert(r.name.clone(), r);
                    }
                }
            }
        }
    }

    let mut list: Vec<Recipe> = map.into_values().collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(list)
}

/// Saves a recipe to either workspace (.chelp/recipes.toml) or global user recipes
pub fn save_recipe(
    recipe: Recipe,
    global: bool,
    workspace_dir: Option<&Path>,
) -> Result<PathBuf, ChelpError> {
    let target_path = if global || workspace_dir.is_none() {
        get_global_recipes_path()
    } else {
        get_workspace_recipes_path(workspace_dir.unwrap())
    };

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut current_book = if target_path.exists() {
        let content = fs::read_to_string(&target_path)?;
        toml::from_str::<RecipeBook>(&content).unwrap_or_default()
    } else {
        RecipeBook::default()
    };

    // Remove existing if any, then append
    current_book.recipe.retain(|r| r.name != recipe.name);
    current_book.recipe.push(recipe);
    current_book.recipe.sort_by(|a, b| a.name.cmp(&b.name));

    let toml_str = toml::to_string_pretty(&current_book)
        .map_err(|e| ChelpError::Config(format!("Failed to serialize recipes: {}", e)))?;
    fs::write(&target_path, toml_str)?;

    Ok(target_path)
}

/// Removes a recipe by name from either workspace or global
pub fn remove_recipe(
    name: &str,
    global: bool,
    workspace_dir: Option<&Path>,
) -> Result<bool, ChelpError> {
    let target_path = if global || workspace_dir.is_none() {
        get_global_recipes_path()
    } else {
        get_workspace_recipes_path(workspace_dir.unwrap())
    };

    if !target_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(&target_path)?;
    let mut book = toml::from_str::<RecipeBook>(&content).unwrap_or_default();
    let initial_len = book.recipe.len();
    book.recipe.retain(|r| r.name != name);

    if book.recipe.len() == initial_len {
        return Ok(false);
    }

    let toml_str = toml::to_string_pretty(&book)
        .map_err(|e| ChelpError::Config(format!("Failed to serialize recipes: {}", e)))?;
    fs::write(&target_path, toml_str)?;

    Ok(true)
}

/// Executes a recipe with interactive confirmation
pub fn run_recipe(name: &str, workspace_dir: Option<&Path>) -> Result<Option<String>, ChelpError> {
    let recipes = load_recipes(workspace_dir)?;
    let recipe = recipes
        .into_iter()
        .find(|r| r.name == name)
        .ok_or_else(|| ChelpError::Config(format!("Recipe '{}' not found. Run 'chelp recipe list'.", name)))?;

    let (safety_level, warning) = recipe.resolved_safety();

    let resp = AiCommandResponse {
        command: recipe.command.clone(),
        explanation: recipe.description.clone(),
        safety_level,
        destructive_warning: warning,
    };

    match render_interactive_confirmation(&resp)? {
        UserAction::Run(cmd) | UserAction::Edit(cmd) => Ok(Some(cmd)),
        UserAction::Cancel => Ok(None),
    }
}
