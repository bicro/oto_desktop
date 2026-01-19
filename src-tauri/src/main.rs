// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Module declarations
mod db;
mod models;
mod paths;
mod prompts;

// Re-exports for internal use
use db::{clear_chat_history_internal, get_chat_history_internal, store_chat_message};
use models::{ChatMessage, ChatResponse, DeepResearchResponse, TextureVersion};
use paths::*;
use prompts::*;

use rdev::{listen, Event, EventType};
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{Read, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder};
use tauri::{command, AppHandle, Emitter, Manager};
#[cfg(target_os = "windows")]
use tauri::http::Response;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
// rusqlite is now used in db.rs module
use log::{error, info, warn};
use serde::Deserialize;

// macOS-specific imports
#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};

// Windows-specific imports
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
};

// Path helper functions are in paths.rs module

// Default prompt constants are in prompts.rs module

// Database functions are in db.rs module
// Model structs are in models.rs module

// ============ Download Helpers ============

async fn download_and_extract_zip(url: &str, dest_dir: &PathBuf) -> Result<(), String> {
    // Download to temp file
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Download failed with status: {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    // Create destination directory
    std::fs::create_dir_all(dest_dir).map_err(|e| format!("Failed to create directory: {}", e))?;

    // Extract zip
    let cursor = std::io::Cursor::new(bytes.to_vec());
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Failed to read zip: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        let outpath = dest_dir.join(file.name());

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directory: {}", e))?;
            }
            let mut outfile = std::fs::File::create(&outpath)
                .map_err(|e| format!("Failed to create file: {}", e))?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)
                .map_err(|e| format!("Failed to read zip content: {}", e))?;
            outfile
                .write_all(&buffer)
                .map_err(|e| format!("Failed to write file: {}", e))?;
        }
    }

    Ok(())
}

// ============ Tauri Commands ============

#[derive(Serialize)]
pub struct InitStatus {
    pub ready: bool,
    pub message: String,
    pub models_path: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModelConfig {
    pub url: String,
    pub folder: String,
    pub model_file: String,
    pub texture_folder: Option<String>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            url: DEFAULT_MODEL_URL.to_string(),
            folder: "Hiyori".to_string(),
            model_file: "Hiyori.model3.json".to_string(),
            texture_folder: Some("Hiyori.2048".to_string()),
        }
    }
}

fn load_model_config() -> Result<ModelConfig, String> {
    let config_path = get_model_config_path()?;
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read model config: {}", e))?;
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse model config: {}", e))
    } else {
        Ok(ModelConfig::default())
    }
}

fn save_model_config(config: &ModelConfig) -> Result<(), String> {
    let config_path = get_model_config_path()?;
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize model config: {}", e))?;
    std::fs::write(&config_path, content).map_err(|e| format!("Failed to save model config: {}", e))
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TransformConfig {
    pub scale: f64,
    pub offset_x: f64,
    pub offset_y: f64,
}

impl Default for TransformConfig {
    fn default() -> Self {
        Self {
            scale: 1.5,
            offset_x: 15.0,
            offset_y: 109.0,
        }
    }
}

#[tauri::command]
fn save_transform_config(scale: f64, offset_x: f64, offset_y: f64) -> Result<(), String> {
    let config_path = paths::get_transform_config_path()?;
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    let config = TransformConfig {
        scale,
        offset_x,
        offset_y,
    };
    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize transform config: {}", e))?;
    std::fs::write(&config_path, content)
        .map_err(|e| format!("Failed to save transform config: {}", e))
}

#[tauri::command]
fn load_transform_config() -> Result<TransformConfig, String> {
    let config_path = paths::get_transform_config_path()?;
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read transform config: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse transform config: {}", e))
    } else {
        Ok(TransformConfig::default())
    }
}

/// Maximum depth to search for model files in nested directories
const MAX_MODEL_SEARCH_DEPTH: u32 = 3;

/// Recursively find a .model3.json file in a directory (up to max_depth levels)
fn find_model_file_recursive(dir: &PathBuf, max_depth: u32) -> Option<(PathBuf, String)> {
    if max_depth == 0 {
        return None;
    }

    let entries: Vec<_> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .collect();

    // First pass: look for model file at this level
    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.path().is_file() && name.ends_with(".model3.json") {
            return Some((dir.clone(), name));
        }
    }

    // Second pass: search subdirectories
    for entry in &entries {
        if entry.path().is_dir() {
            if let Some(result) = find_model_file_recursive(&entry.path(), max_depth - 1) {
                return Some(result);
            }
        }
    }

    None
}

/// Reorganize flat model files into a subdirectory
/// Called when a zip extracts files directly without a wrapper folder
fn reorganize_flat_model(models_dir: &PathBuf, model_filename: &str) -> Result<String, String> {
    let model_name = model_filename.trim_end_matches(".model3.json");
    let new_folder = models_dir.join(model_name);

    std::fs::create_dir_all(&new_folder)
        .map_err(|e| format!("Failed to create model folder: {}", e))?;

    // Move all files from models_dir to the new subfolder
    let entries: Vec<_> = std::fs::read_dir(models_dir)
        .map_err(|e| format!("Failed to read models dir: {}", e))?
        .filter_map(|e| e.ok())
        .collect();

    for entry in entries {
        let file_path = entry.path();

        // Skip the folder we just created
        if file_path == new_folder {
            continue;
        }

        let dest = new_folder.join(entry.file_name());
        std::fs::rename(&file_path, &dest)
            .map_err(|e| format!("Failed to move {:?}: {}", entry.file_name(), e))?;
    }

    Ok(model_name.to_string())
}

/// Auto-detect model structure after extraction
fn detect_model_structure(
    models_dir: &PathBuf,
) -> Result<(String, String, Option<String>), String> {
    let entries: Vec<_> = std::fs::read_dir(models_dir)
        .map_err(|e| format!("Failed to read models directory: {}", e))?
        .filter_map(|e| e.ok())
        .collect();

    // Check if model file is directly in models_dir (flat zip structure)
    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.path().is_file() && name.ends_with(".model3.json") {
            let model_name = reorganize_flat_model(models_dir, &name)?;
            let model_folder = models_dir.join(&model_name);
            let texture_folder = find_texture_folder(&model_folder);
            return Ok((model_name, name, texture_folder));
        }
    }

    // Search subdirectories for model files
    for entry in entries {
        if !entry.path().is_dir() {
            continue;
        }

        let folder_path = entry.path();

        if let Some((model_dir, model_file)) =
            find_model_file_recursive(&folder_path, MAX_MODEL_SEARCH_DEPTH)
        {
            let texture_folder = find_texture_folder(&model_dir);
            let relative_path = model_dir
                .strip_prefix(models_dir)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| entry.file_name().to_string_lossy().to_string());

            return Ok((relative_path, model_file, texture_folder));
        }
    }

    Err("No Live2D model found in extracted files".to_string())
}

/// Find texture folder within a model directory
fn find_texture_folder(model_dir: &PathBuf) -> Option<String> {
    if let Ok(entries) = std::fs::read_dir(model_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.path().is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                // Common Live2D texture folder patterns
                if name.ends_with(".2048")
                    || name.ends_with(".4096")
                    || name.ends_with(".1024")
                    || name == "textures"
                {
                    return Some(name);
                }
            }
        }
        // Fallback: look for folder containing .png files
        for entry in std::fs::read_dir(model_dir).ok()?.filter_map(|e| e.ok()) {
            if entry.path().is_dir() {
                let dir_path = entry.path();
                if let Ok(files) = std::fs::read_dir(&dir_path) {
                    for file in files.filter_map(|f| f.ok()) {
                        if file.file_name().to_string_lossy().ends_with(".png") {
                            return Some(entry.file_name().to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

#[command]
async fn init_app(app: AppHandle) -> Result<InitStatus, String> {
    let models_dir = get_models_dir()?;

    println!("[init_app] Starting initialization...");
    println!("[init_app] Models dir: {:?}", models_dir);

    // Load or create model config
    let mut config = load_model_config().unwrap_or_default();

    // Emit progress events to frontend
    let emit_progress = |step: &str, message: &str| {
        println!("[init_app] {}: {}", step, message);
        let _ = app.emit("init-progress", json!({ "step": step, "message": message }));
    };

    // Check if model exists
    let model_dir = models_dir.join(&config.folder);
    if !model_dir.exists() {
        emit_progress("model", "Downloading model...");
        match download_and_extract_zip(&config.url, &models_dir).await {
            Ok(_) => {
                // Auto-detect model structure after download
                match detect_model_structure(&models_dir) {
                    Ok((folder, model_file, texture_folder)) => {
                        config.folder = folder;
                        config.model_file = model_file;
                        config.texture_folder = texture_folder;
                        save_model_config(&config)?;
                        emit_progress("model", "Model ready!");
                    }
                    Err(e) => {
                        println!(
                            "[init_app] WARNING: Could not detect model structure: {}",
                            e
                        );
                        // Use defaults for the default model
                        save_model_config(&config)?;
                        emit_progress("model", "Model ready!");
                    }
                }
            }
            Err(e) => {
                println!("[init_app] ERROR downloading model: {}", e);
                return Err(format!("Failed to download model: {}", e));
            }
        }
    } else {
        println!(
            "[init_app] Model already exists at {:?}, skipping download",
            model_dir
        );
        // Ensure config is saved even if model already exists
        if !get_model_config_path()?.exists() {
            save_model_config(&config)?;
        }
    }

    emit_progress("done", "All ready!");
    println!("[init_app] Initialization complete!");

    Ok(InitStatus {
        ready: true,
        message: "Ready".to_string(),
        models_path: models_dir.to_string_lossy().to_string(),
    })
}

#[command]
async fn get_paths() -> Result<String, String> {
    let models_dir = get_models_dir()?;
    let path_str = models_dir.to_string_lossy().to_string();
    info!("[get_paths] Models directory: {}", path_str);
    Ok(path_str)
}

#[command]
async fn read_file_as_text(path: String) -> Result<String, String> {
    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Failed to read file {}: {}", path, e))
}

#[command]
async fn read_file_as_bytes(path: String) -> Result<Vec<u8>, String> {
    tokio::fs::read(&path)
        .await
        .map_err(|e| format!("Failed to read file {}: {}", path, e))
}

#[command]
async fn is_initialized() -> Result<bool, String> {
    let models_dir = get_models_dir()?;
    let config = load_model_config().unwrap_or_default();
    Ok(models_dir.join(&config.folder).exists())
}

// ============ Model Config Commands ============

#[command]
async fn get_model_config() -> Result<ModelConfig, String> {
    let config = load_model_config()?;
    info!(
        "[get_model_config] Loaded config - folder: {}, model_file: {}",
        config.folder, config.model_file
    );
    Ok(config)
}

#[command]
async fn change_model(app: AppHandle, url: String) -> Result<ModelConfig, String> {
    let models_dir = get_models_dir()?;

    println!("[change_model] Changing model to: {}", url);

    // Reset zoom to 100% for new model
    save_overlay_scale_to_file(1.0)?;

    // Emit progress
    let _ = app.emit(
        "model-change-progress",
        json!({ "status": "downloading", "message": "Downloading new model..." }),
    );

    // Clear existing models
    if models_dir.exists() {
        std::fs::remove_dir_all(&models_dir)
            .map_err(|e| format!("Failed to clear models directory: {}", e))?;
    }
    std::fs::create_dir_all(&models_dir)
        .map_err(|e| format!("Failed to create models directory: {}", e))?;

    // Download and extract new model
    download_and_extract_zip(&url, &models_dir).await?;

    let _ = app.emit(
        "model-change-progress",
        json!({ "status": "detecting", "message": "Detecting model structure..." }),
    );

    // Detect model structure
    let (folder, model_file, texture_folder) = detect_model_structure(&models_dir)?;

    // Save new config
    let config = ModelConfig {
        url: url.clone(),
        folder,
        model_file,
        texture_folder,
    };
    save_model_config(&config)?;

    let _ = app.emit(
        "model-change-progress",
        json!({ "status": "complete", "message": "Model changed successfully!" }),
    );

    // Notify frontend of scale reset
    let _ = app.emit("overlay-scale-reset", json!({ "scale": 1.0 }));

    println!("[change_model] Model changed successfully: {:?}", config);

    Ok(config)
}

#[command]
async fn reset_model(app: AppHandle) -> Result<ModelConfig, String> {
    // Reset to default model
    change_model(app, DEFAULT_MODEL_URL.to_string()).await
}

#[command]
async fn load_model_from_folder(
    app: AppHandle,
    folder_path: String,
) -> Result<ModelConfig, String> {
    let models_dir = get_models_dir()?;
    let source_path = PathBuf::from(&folder_path);

    println!("[load_model_from_folder] Loading from: {}", folder_path);

    // Validate source folder exists
    if !source_path.exists() || !source_path.is_dir() {
        return Err("Selected path is not a valid folder".to_string());
    }

    // Validate it contains a .model3.json file
    let has_model = std::fs::read_dir(&source_path)
        .map_err(|e| format!("Failed to read folder: {}", e))?
        .filter_map(|e| e.ok())
        .any(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            name.ends_with(".model3.json")
        });

    if !has_model {
        // Check subdirectories
        let has_model_nested = std::fs::read_dir(&source_path)
            .map_err(|e| format!("Failed to read folder: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .any(|dir_entry| {
                find_model_file_recursive(&dir_entry.path(), MAX_MODEL_SEARCH_DEPTH).is_some()
            });

        if !has_model_nested {
            return Err("No Live2D model (.model3.json) found in folder".to_string());
        }
    }

    // Reset zoom to 100% for new model
    save_overlay_scale_to_file(1.0)?;

    // Emit progress
    let _ = app.emit(
        "model-change-progress",
        json!({ "status": "copying", "message": "Copying model files..." }),
    );

    // Clear existing models
    if models_dir.exists() {
        std::fs::remove_dir_all(&models_dir)
            .map_err(|e| format!("Failed to clear models directory: {}", e))?;
    }
    std::fs::create_dir_all(&models_dir)
        .map_err(|e| format!("Failed to create models directory: {}", e))?;

    // Copy entire folder to models directory
    copy_dir_recursive(&source_path, &models_dir)?;

    let _ = app.emit(
        "model-change-progress",
        json!({ "status": "detecting", "message": "Detecting model structure..." }),
    );

    // Detect model structure (reuses existing function)
    let (folder, model_file, texture_folder) = detect_model_structure(&models_dir)?;

    // Save config with "local:" prefix to indicate local source
    let config = ModelConfig {
        url: format!("local:{}", folder_path),
        folder,
        model_file,
        texture_folder,
    };
    save_model_config(&config)?;

    let _ = app.emit(
        "model-change-progress",
        json!({ "status": "complete", "message": "Model loaded successfully!" }),
    );

    let _ = app.emit("overlay-scale-reset", json!({ "scale": 1.0 }));

    println!("[load_model_from_folder] Model loaded: {:?}", config);

    Ok(config)
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    for entry in std::fs::read_dir(src).map_err(|e| format!("Failed to read dir: {}", e))? {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            std::fs::create_dir_all(&dst_path)
                .map_err(|e| format!("Failed to create dir: {}", e))?;
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("Failed to copy file: {}", e))?;
        }
    }
    Ok(())
}

// ============ API Key Commands ============

#[command]
async fn save_api_key(key: String) -> Result<(), String> {
    info!("[save_api_key] Starting to save API key");
    let key_path = get_api_key_path()?;
    info!("[save_api_key] Key path: {:?}", key_path);

    // Ensure parent directory exists
    if let Some(parent) = key_path.parent() {
        info!("[save_api_key] Creating parent directory: {:?}", parent);
        std::fs::create_dir_all(parent).map_err(|e| {
            error!("[save_api_key] Failed to create directory: {}", e);
            format!("Failed to create directory: {}", e)
        })?;
    }

    std::fs::write(&key_path, &key).map_err(|e| {
        error!("[save_api_key] Failed to save API key: {}", e);
        format!("Failed to save API key: {}", e)
    })?;

    info!("[save_api_key] API key saved successfully");
    Ok(())
}

#[command]
async fn get_api_key() -> Result<Option<String>, String> {
    let key_path = get_api_key_path()?;

    if key_path.exists() {
        let key = std::fs::read_to_string(&key_path)
            .map_err(|e| format!("Failed to read API key: {}", e))?;
        Ok(Some(key.trim().to_string()))
    } else {
        Ok(None)
    }
}

#[command]
async fn has_api_key() -> Result<bool, String> {
    let key_path = get_api_key_path()?;
    Ok(key_path.exists())
}

// ============ Prompt Commands ============

#[command]
async fn save_system_prompt(prompt: String) -> Result<(), String> {
    let prompt_path = get_system_prompt_path()?;

    if let Some(parent) = prompt_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    std::fs::write(&prompt_path, &prompt)
        .map_err(|e| format!("Failed to save system prompt: {}", e))?;

    Ok(())
}

#[command]
async fn get_system_prompt() -> Result<String, String> {
    let prompt_path = get_system_prompt_path()?;

    if prompt_path.exists() {
        let prompt = std::fs::read_to_string(&prompt_path)
            .map_err(|e| format!("Failed to read system prompt: {}", e))?;
        let trimmed = prompt.trim().to_string();
        if trimmed.is_empty() {
            Ok(DEFAULT_SYSTEM_PROMPT.to_string())
        } else {
            Ok(trimmed)
        }
    } else {
        Ok(DEFAULT_SYSTEM_PROMPT.to_string())
    }
}

#[command]
async fn save_character_prompt(prompt: String) -> Result<(), String> {
    let prompt_path = get_character_prompt_path()?;

    if let Some(parent) = prompt_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    std::fs::write(&prompt_path, &prompt)
        .map_err(|e| format!("Failed to save character prompt: {}", e))?;

    Ok(())
}

#[command]
async fn get_character_prompt() -> Result<String, String> {
    let prompt_path = get_character_prompt_path()?;

    if prompt_path.exists() {
        let prompt = std::fs::read_to_string(&prompt_path)
            .map_err(|e| format!("Failed to read character prompt: {}", e))?;
        let trimmed = prompt.trim().to_string();
        if trimmed.is_empty() {
            Ok(DEFAULT_CHARACTER_PROMPT.to_string())
        } else {
            Ok(trimmed)
        }
    } else {
        Ok(DEFAULT_CHARACTER_PROMPT.to_string())
    }
}

#[command]
async fn save_deep_research_prompt(prompt: String) -> Result<(), String> {
    let prompt_path = get_deep_research_prompt_path()?;

    if let Some(parent) = prompt_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    std::fs::write(&prompt_path, &prompt)
        .map_err(|e| format!("Failed to save deep research prompt: {}", e))?;

    Ok(())
}

#[command]
async fn get_deep_research_prompt() -> Result<String, String> {
    let prompt_path = get_deep_research_prompt_path()?;

    if prompt_path.exists() {
        let prompt = std::fs::read_to_string(&prompt_path)
            .map_err(|e| format!("Failed to read deep research prompt: {}", e))?;
        let trimmed = prompt.trim().to_string();
        if trimmed.is_empty() {
            Ok(DEFAULT_DEEP_RESEARCH_PROMPT.to_string())
        } else {
            Ok(trimmed)
        }
    } else {
        Ok(DEFAULT_DEEP_RESEARCH_PROMPT.to_string())
    }
}

#[command]
async fn save_dialogue_prompt(prompt: String) -> Result<(), String> {
    let prompt_path = get_dialogue_prompt_path()?;

    if let Some(parent) = prompt_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    std::fs::write(&prompt_path, &prompt)
        .map_err(|e| format!("Failed to save dialogue prompt: {}", e))?;

    Ok(())
}

#[command]
async fn get_dialogue_prompt() -> Result<String, String> {
    let prompt_path = get_dialogue_prompt_path()?;

    if prompt_path.exists() {
        let prompt = std::fs::read_to_string(&prompt_path)
            .map_err(|e| format!("Failed to read dialogue prompt: {}", e))?;
        let trimmed = prompt.trim().to_string();
        if trimmed.is_empty() {
            Ok(DEFAULT_DIALOGUE_PROMPT.to_string())
        } else {
            Ok(trimmed)
        }
    } else {
        Ok(DEFAULT_DIALOGUE_PROMPT.to_string())
    }
}

// ============ Frontend Logging ============

#[command]
fn log_from_frontend(level: String, message: String) {
    match level.as_str() {
        "error" => error!("[Frontend] {}", message),
        "warn" => warn!("[Frontend] {}", message),
        _ => info!("[Frontend] {}", message),
    }
}

// ============ Hitbox Commands ============

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Point2D {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HitboxData {
    points: Vec<Point2D>,
}

#[command]
async fn save_hitbox(points: Vec<Point2D>) -> Result<(), String> {
    let hitbox_path = get_hitbox_path()?;

    if let Some(parent) = hitbox_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    let data = HitboxData { points };
    let json = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("Failed to serialize hitbox: {}", e))?;

    std::fs::write(&hitbox_path, json).map_err(|e| format!("Failed to save hitbox: {}", e))?;

    println!("[Hitbox] Saved {} points", data.points.len());
    Ok(())
}

#[command]
async fn load_hitbox() -> Result<Option<HitboxData>, String> {
    let hitbox_path = get_hitbox_path()?;

    if !hitbox_path.exists() {
        return Ok(None);
    }

    let json = std::fs::read_to_string(&hitbox_path)
        .map_err(|e| format!("Failed to read hitbox: {}", e))?;

    let data: HitboxData =
        serde_json::from_str(&json).map_err(|e| format!("Failed to parse hitbox: {}", e))?;

    println!("[Hitbox] Loaded {} points", data.points.len());
    Ok(Some(data))
}

#[command]
async fn clear_hitbox() -> Result<(), String> {
    let hitbox_path = get_hitbox_path()?;

    if hitbox_path.exists() {
        std::fs::remove_file(&hitbox_path).map_err(|e| format!("Failed to clear hitbox: {}", e))?;
        println!("[Hitbox] Cleared hitbox");
    }

    Ok(())
}

// ============ Chat Commands ============

#[command]
async fn send_chat_message(
    app: AppHandle,
    message: String,
    include_screenshot: bool,
    context_level: u8,
) -> Result<ChatResponse, String> {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

    // Get API key
    let api_key = get_api_key()
        .await?
        .ok_or_else(|| "API key not configured".to_string())?;

    // Get system prompt based on level
    let system_prompt = match context_level {
        1 => {
            // Level 1: Use dialogue prompt (respond AS the character in direct conversation)
            get_dialogue_prompt().await?
        }
        2 => {
            // Level 2: Use deep research prompt (respond as analyst)
            get_deep_research_prompt().await?
        }
        _ => {
            // Level 0: Default system prompt
            get_system_prompt().await?
        }
    };

    // Take screenshot if enabled (only for level 0)
    let screenshot_base64 = if include_screenshot && context_level == 0 {
        let screenshot_path = take_screenshot(app).await?;
        let screenshot_bytes = std::fs::read(&screenshot_path)
            .map_err(|e| format!("Failed to read screenshot: {}", e))?;
        Some(BASE64.encode(&screenshot_bytes))
    } else {
        None
    };

    // Get recent chat history for context
    let history = get_chat_history_internal(10)?;

    // Build messages array with system prompt
    let mut messages: Vec<Value> = vec![json!({
        "role": "system",
        "content": system_prompt
    })];

    // Add past messages for context, filtered by level
    for msg in &history {
        let include_msg = match context_level {
            1 => {
                // Level 1: User + character + assistant (includes AI responses for context)
                msg.role == "user" || msg.role == "character" || msg.role == "assistant"
            }
            2 => {
                // Level 2: Only user + deep-thought messages
                msg.role == "user" || msg.role == "deep-thought"
            }
            _ => {
                // Level 0: All except deep-thought
                msg.role != "deep-thought"
            }
        };

        if !include_msg {
            continue;
        }

        // Convert custom roles to "assistant" for API compatibility
        // Add distinct labels for level 1 context so character knows what's what
        let (role, content) = if msg.role == "character" {
            if context_level == 1 {
                (
                    "assistant",
                    format!("[Character's Inner Thoughts]: {}", msg.content),
                )
            } else {
                ("assistant", format!("[Character]: {}", msg.content))
            }
        } else if msg.role == "assistant" && context_level == 1 {
            // For Level 1, format assistant messages distinctly
            (
                "assistant",
                format!("[AI Assistant Response]: {}", msg.content),
            )
        } else if msg.role == "deep-thought" {
            ("assistant", format!("[Analysis]: {}", msg.content))
        } else {
            (msg.role.as_str(), msg.content.clone())
        };

        messages.push(json!({
            "role": role,
            "content": content
        }));
    }

    // Add current message (with or without screenshot)
    if let Some(ref base64) = screenshot_base64 {
        messages.push(json!({
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": message.clone()
                },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:image/png;base64,{}", base64)
                    }
                }
            ]
        }));
    } else {
        messages.push(json!({
            "role": "user",
            "content": message.clone()
        }));
    }

    // Call OpenAI API for main response
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": "gpt-4.1-2025-04-14",
            "messages": messages,
            "max_tokens": 1000
        }))
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("API error: {}", error_text));
    }

    let response_json: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let main_response = response_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("No response")
        .to_string();

    // Store messages and generate character comments based on level
    let timestamp = chrono::Utc::now().to_rfc3339();
    store_chat_message(&timestamp, "user", &message, context_level)?;

    let character_comments = match context_level {
        1 => {
            // Level 1: Save response as "character", no separate character comments
            store_chat_message(&timestamp, "character", &main_response, 1)?;
            None
        }
        2 => {
            // Level 2: Save response as "deep-thought", no character comments
            store_chat_message(&timestamp, "deep-thought", &main_response, 2)?;
            None
        }
        _ => {
            // Level 0: Save as "assistant", then generate character comment
            store_chat_message(&timestamp, "assistant", &main_response, 0)?;

            // Generate character commentary for level 0 only
            let char_system_prompt = get_character_prompt().await?;

            let char_messages: Vec<Value> = vec![
                json!({
                    "role": "system",
                    "content": char_system_prompt
                }),
                json!({
                    "role": "user",
                    "content": format!("Here is the AI response to comment on:\n\n{}", main_response)
                }),
            ];

            let char_response = client
                .post("https://api.openai.com/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&json!({
                    "model": "gpt-4.1-2025-04-14",
                    "messages": char_messages,
                    "max_tokens": 500
                }))
                .send()
                .await;

            match char_response {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(char_json) = resp.json::<Value>().await {
                        let char_content = char_json["choices"][0]["message"]["content"]
                            .as_str()
                            .unwrap_or("");
                        if !char_content.is_empty() {
                            // Store character comment at level 0
                            store_chat_message(&timestamp, "character", char_content, 0)?;
                            // Return as single comment at end (not randomly inserted)
                            Some(vec![char_content.trim().to_string()])
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
    };

    Ok(ChatResponse {
        main_response,
        character_comments,
    })
}

// Database helper functions (store_chat_message, get_chat_history_internal) are in db.rs

#[command]
async fn get_chat_history() -> Result<Vec<ChatMessage>, String> {
    get_chat_history_internal(100)
}

#[command]
async fn clear_chat_history() -> Result<(), String> {
    clear_chat_history_internal()
}

#[command]
async fn trigger_deep_research() -> Result<DeepResearchResponse, String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let cooldown_path = get_deep_research_cooldown_path()?;
    let six_hours: u64 = 6 * 60 * 60;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();

    // Check cooldown
    if cooldown_path.exists() {
        let last_time_str = std::fs::read_to_string(&cooldown_path).map_err(|e| e.to_string())?;
        if let Ok(last_time) = last_time_str.parse::<u64>() {
            if now - last_time < six_hours {
                let remaining = six_hours - (now - last_time);
                // Return cooldown status - frontend will show timer and existing deep thought
                return Ok(DeepResearchResponse {
                    on_cooldown: true,
                    remaining_seconds: remaining,
                    main_response: String::new(),
                });
            }
        }
    }

    // Not on cooldown - run deep research
    let api_key = get_api_key().await?.ok_or("API key not configured")?;
    let deep_prompt = get_deep_research_prompt().await?;
    let history = get_chat_history_internal(50)?;

    let context = history
        .iter()
        .map(|m| format!("[{}]: {}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n\n");

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "gpt-4o",
            "messages": [
                { "role": "system", "content": deep_prompt },
                { "role": "user", "content": format!("Analyze this conversation history:\n\n{}", context) }
            ]
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        error!("[DeepResearch] API error: {}", error_text);
        return Err(format!("API request failed: {}", error_text));
    }

    let response_json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let insights = response_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("No insights generated")
        .to_string();

    // Store with deep-thought marker at level 2
    let timestamp = chrono::Utc::now().to_rfc3339();
    store_chat_message(&timestamp, "deep-thought", &insights, 2)?;

    // Update cooldown timestamp
    if let Some(parent) = cooldown_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&cooldown_path, now.to_string()).map_err(|e| e.to_string())?;

    Ok(DeepResearchResponse {
        on_cooldown: false,
        remaining_seconds: 0,
        main_response: insights,
    })
}

#[command]
async fn clear_all_data() -> Result<(), String> {
    clear_app_data()
}

#[command]
async fn generate_texture(prompt: String) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
    use image::GenericImageView;

    // Load model config for dynamic paths
    let config = load_model_config()?;
    let texture_folder = config
        .texture_folder
        .ok_or_else(|| "No texture folder configured for this model".to_string())?;
    let texture_dir = get_texture_dir_for_model(&config.folder, &texture_folder)?;
    let originals_dir = get_originals_dir_for_model(&config.folder, &texture_folder)?;

    // Get OpenAI API key
    let api_key = get_api_key()
        .await?
        .ok_or_else(|| "No API key configured".to_string())?;

    // Discover texture files dynamically
    let texture_files: Vec<String> = std::fs::read_dir(&texture_dir)
        .map_err(|e| format!("Failed to read texture directory: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "png"))
        .filter(|e| !e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    if texture_files.is_empty() {
        return Err("No texture files found in model".to_string());
    }

    for texture_file in &texture_files {
        let texture_path = texture_dir.join(texture_file);
        let original_path = originals_dir.join(texture_file);

        // Ensure we have originals backed up first
        if !original_path.exists() {
            if texture_path.exists() {
                std::fs::create_dir_all(&originals_dir)
                    .map_err(|e| format!("Failed to create originals dir: {}", e))?;
                std::fs::copy(&texture_path, &original_path)
                    .map_err(|e| format!("Failed to backup {}: {}", texture_file, e))?;
            } else {
                continue;
            }
        }

        // Load the original image
        let img = image::open(&original_path)
            .map_err(|e| format!("Failed to load {}: {}", texture_file, e))?;

        let (orig_width, orig_height) = img.dimensions();
        println!(
            "[Texture] Processing {} - original dimensions: {}x{}",
            texture_file, orig_width, orig_height
        );

        // Downscale to 1024x1024 for OpenAI
        println!("[Texture] Downscaling to 1024x1024...");
        let downscaled = img.resize_exact(1024, 1024, image::imageops::FilterType::Lanczos3);

        // Encode as PNG bytes
        let mut png_bytes: Vec<u8> = Vec::new();
        downscaled
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .map_err(|e| format!("Failed to encode image: {}", e))?;

        // Create multipart form for OpenAI API
        let form = reqwest::multipart::Form::new()
            .text("model", "gpt-image-1.5")
            .text(
                "prompt",
                format!(
                    "This is a texture atlas for a Live2D anime character. {}. \
                CRITICAL: Keep every element in its EXACT position. \
                Preserve all black outlines/lineart. \
                Only modify what the prompt asks for. \
                Maintain the same art style and quality.",
                    prompt
                ),
            )
            .text("size", "1024x1024")
            .text("background", "transparent")
            .text("output_format", "png")
            .part(
                "image[]",
                reqwest::multipart::Part::bytes(png_bytes)
                    .file_name("texture.png")
                    .mime_str("image/png")
                    .map_err(|e| format!("Failed to set mime type: {}", e))?,
            );

        // Call OpenAI API
        println!("[Texture] Sending to OpenAI...");
        let client = reqwest::Client::new();
        let response = client
            .post("https://api.openai.com/v1/images/edits")
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("OpenAI API failed for {}: {}", texture_file, e))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!(
                "OpenAI API error for {}: {}",
                texture_file, error_text
            ));
        }

        let response_json: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response for {}: {}", texture_file, e))?;

        println!("[Texture] Response received, extracting image...");

        // Extract base64 image from response
        let image_data = response_json["data"][0]["b64_json"]
            .as_str()
            .ok_or_else(|| format!("No image in response for {}", texture_file))?;

        // Decode the edited image
        let decoded = BASE64
            .decode(image_data)
            .map_err(|e| format!("Failed to decode {}: {}", texture_file, e))?;

        let edited_img = image::load_from_memory(&decoded)
            .map_err(|e| format!("Failed to load edited {}: {}", texture_file, e))?;

        // Upscale back to original dimensions (2048x2048)
        println!(
            "[Texture] Upscaling back to {}x{}...",
            orig_width, orig_height
        );
        let upscaled = edited_img.resize_exact(
            orig_width,
            orig_height,
            image::imageops::FilterType::Lanczos3,
        );

        // Save the upscaled image
        upscaled
            .save(&texture_path)
            .map_err(|e| format!("Failed to save {}: {}", texture_file, e))?;

        println!("[Texture] {} completed successfully", texture_file);
    }

    // Save this generation as a version
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let version_dir = get_versions_dir_for_model(&config.folder, &texture_folder)?.join(&timestamp);
    std::fs::create_dir_all(&version_dir)
        .map_err(|e| format!("Failed to create version dir: {}", e))?;

    // Copy all processed textures to the version folder
    for texture_file in &texture_files {
        let src = texture_dir.join(texture_file);
        let dst = version_dir.join(texture_file);
        if src.exists() {
            std::fs::copy(&src, &dst)
                .map_err(|e| format!("Failed to copy {} to version: {}", texture_file, e))?;
        }
    }

    // Save metadata
    let metadata = json!({
        "timestamp": timestamp,
        "prompt": prompt,
        "created_at": chrono::Utc::now().to_rfc3339()
    });
    std::fs::write(version_dir.join("metadata.json"), metadata.to_string())
        .map_err(|e| format!("Failed to save metadata: {}", e))?;

    Ok("Texture generated successfully!".to_string())
}

#[derive(Serialize)]
pub struct TexturePaths {
    pub current_textures: Vec<String>,
    pub original_textures: Vec<String>,
    pub has_original: bool,
    pub texture_enabled: bool,
}

#[command]
async fn get_texture_paths() -> Result<TexturePaths, String> {
    let config = load_model_config()?;

    // Check if texture editing is enabled for this model
    let texture_folder = match &config.texture_folder {
        Some(folder) => folder.clone(),
        None => {
            return Ok(TexturePaths {
                current_textures: vec![],
                original_textures: vec![],
                has_original: false,
                texture_enabled: false,
            });
        }
    };

    let texture_dir = get_texture_dir_for_model(&config.folder, &texture_folder)?;
    let originals_dir = get_originals_dir_for_model(&config.folder, &texture_folder)?;

    // Discover current textures
    let current_textures: Vec<String> = if texture_dir.exists() {
        std::fs::read_dir(&texture_dir)
            .map_err(|e| format!("Failed to read texture directory: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "png"))
            .filter(|e| !e.path().is_dir())
            .map(|e| e.path().to_string_lossy().to_string())
            .collect()
    } else {
        vec![]
    };

    // Discover original textures
    let original_textures: Vec<String> = if originals_dir.exists() {
        std::fs::read_dir(&originals_dir)
            .map_err(|e| format!("Failed to read originals directory: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "png"))
            .filter(|e| !e.path().is_dir())
            .map(|e| e.path().to_string_lossy().to_string())
            .collect()
    } else {
        vec![]
    };

    Ok(TexturePaths {
        has_original: !original_textures.is_empty(),
        current_textures,
        original_textures,
        texture_enabled: true,
    })
}

#[command]
async fn reload_character(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    println!("[Rust] reload_character called");

    // Close existing overlay window if it exists
    if let Some(overlay) = app.get_webview_window("overlay") {
        println!("[Rust] Closing existing overlay window");
        overlay
            .close()
            .map_err(|e| format!("Failed to close overlay: {}", e))?;

        // Small delay to ensure window is closed
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    println!("[Rust] Creating new overlay window");

    // Load saved scale (default 1.0)
    let scale = load_overlay_scale();
    let width = paths::DEFAULT_OVERLAY_WIDTH * scale;
    let height = paths::DEFAULT_OVERLAY_HEIGHT * scale;

    // Recreate the overlay window with fresh state
    let overlay = tauri::WebviewWindowBuilder::new(
        &app,
        "overlay",
        tauri::WebviewUrl::App("overlay.html".into()),
    )
    .title("Overlay")
    .visible(false)
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .inner_size(width, height)
    .resizable(true)
    .build()
    .map_err(|e| format!("Failed to create overlay window: {}", e))?;

    println!("[Rust] New overlay window created, configuring...");

    // Configure the overlay (make it click-through, etc.)
    configure_overlay(&overlay)?;

    // Position in bottom right of screen
    if let Ok(Some(monitor)) = overlay.current_monitor() {
        let screen_size = monitor.size();
        let screen_pos = monitor.position();
        if let Ok(window_size) = overlay.outer_size() {
            let x = screen_pos.x + (screen_size.width as i32) - (window_size.width as i32);
            let y = screen_pos.y + (screen_size.height as i32) - (window_size.height as i32);
            let _ =
                overlay.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
        }
    }

    // Show the overlay
    overlay
        .show()
        .map_err(|e| format!("Failed to show overlay: {}", e))?;

    // Update state
    *state.overlay_visible.lock().unwrap() = true;

    // Wait for page to fully load before emitting init-complete
    println!("[Rust] Waiting for overlay page to load...");
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Emit init-complete to trigger model loading
    println!("[Rust] Emitting init-complete to load model");
    overlay
        .emit("init-complete", json!({}))
        .map_err(|e| format!("Failed to emit init-complete: {}", e))?;

    println!("[Rust] Overlay recreated successfully");
    Ok("Character reloaded!".to_string())
}

// TextureVersion struct is in models.rs

#[command]
async fn get_texture_versions() -> Result<Vec<TextureVersion>, String> {
    let config = load_model_config()?;
    let texture_folder = config
        .texture_folder
        .ok_or_else(|| "No texture folder configured".to_string())?;

    let versions_dir = get_versions_dir_for_model(&config.folder, &texture_folder)?;
    let originals_dir = get_originals_dir_for_model(&config.folder, &texture_folder)?;

    let mut versions = Vec::new();

    // Add generated versions
    if versions_dir.exists() {
        for entry in std::fs::read_dir(&versions_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.path().is_dir() {
                let id = entry.file_name().to_string_lossy().to_string();
                let metadata_path = entry.path().join("metadata.json");
                let (created_at, prompt) = if metadata_path.exists() {
                    let content = std::fs::read_to_string(&metadata_path).unwrap_or_default();
                    let json: Value = serde_json::from_str(&content).unwrap_or(json!({}));
                    (
                        json["created_at"].as_str().unwrap_or(&id).to_string(),
                        json["prompt"].as_str().map(|s| s.to_string()),
                    )
                } else {
                    (id.clone(), None)
                };
                versions.push(TextureVersion {
                    id,
                    created_at,
                    prompt,
                });
            }
        }
    }

    versions.sort_by(|a, b| b.id.cmp(&a.id)); // Newest first

    // Add "original" as the last option if originals exist
    if originals_dir.exists() {
        // Check if any png files exist in originals
        let has_originals = std::fs::read_dir(&originals_dir)
            .ok()
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .any(|e| e.path().extension().is_some_and(|ext| ext == "png"))
            })
            .unwrap_or(false);

        if has_originals {
            versions.push(TextureVersion {
                id: "original".to_string(),
                created_at: "Original".to_string(),
                prompt: Some("Original textures".to_string()),
            });
        }
    }

    Ok(versions)
}

#[command]
async fn apply_texture_version(version_id: String) -> Result<String, String> {
    let config = load_model_config()?;
    let texture_folder = config
        .texture_folder
        .ok_or_else(|| "No texture folder configured".to_string())?;

    let texture_dir = get_texture_dir_for_model(&config.folder, &texture_folder)?;

    // Handle "original" as a special case
    let source_dir = if version_id == "original" {
        get_originals_dir_for_model(&config.folder, &texture_folder)?
    } else {
        get_versions_dir_for_model(&config.folder, &texture_folder)?.join(&version_id)
    };

    if !source_dir.exists() {
        return Err("Version not found".to_string());
    }

    // Discover and copy all texture files from the source
    for entry in
        std::fs::read_dir(&source_dir).map_err(|e| format!("Failed to read source: {}", e))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.path().extension().is_some_and(|ext| ext == "png") {
            let file_name = entry.file_name();
            let dst = texture_dir.join(&file_name);
            std::fs::copy(entry.path(), &dst)
                .map_err(|e| format!("Failed to apply {:?}: {}", file_name, e))?;
        }
    }

    Ok(format!(
        "Applied {}",
        if version_id == "original" {
            "original textures"
        } else {
            &version_id
        }
    ))
}

#[command]
async fn delete_texture_version(version_id: String) -> Result<String, String> {
    // Prevent deleting the original
    if version_id == "original" {
        return Err("Cannot delete original textures".to_string());
    }

    let config = load_model_config()?;
    let texture_folder = config
        .texture_folder
        .ok_or_else(|| "No texture folder configured".to_string())?;

    let versions_dir = get_versions_dir_for_model(&config.folder, &texture_folder)?;
    let version_path = versions_dir.join(&version_id);

    if !version_path.exists() {
        return Err("Version not found".to_string());
    }

    std::fs::remove_dir_all(&version_path).map_err(|e| format!("Failed to delete: {}", e))?;

    Ok("Version deleted".to_string())
}

// ============ App State ============

#[derive(Default)]
pub struct AppState {
    pub overlay_visible: Mutex<bool>,
    pub toggle_menu_item: Mutex<Option<MenuItem<tauri::Wry>>>,
}

// ============ Overlay Window Commands ============

#[cfg(target_os = "macos")]
fn configure_overlay(window: &tauri::WebviewWindow) -> Result<(), String> {
    window
        .with_webview(|webview| unsafe {
            let ns_window_ptr = webview.ns_window();
            let ns_window: Retained<NSWindow> =
                Retained::retain(ns_window_ptr as *mut NSWindow).unwrap();

            let behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary;
            ns_window.setCollectionBehavior(behavior);
            ns_window.setLevel(1000);
        })
        .map_err(|e| format!("Failed to configure overlay: {}", e))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn configure_overlay(window: &tauri::WebviewWindow) -> Result<(), String> {
    let hwnd = window
        .hwnd()
        .map_err(|e| format!("Failed to get HWND: {}", e))?;
    unsafe {
        SetWindowPos(
            HWND(hwnd.0),
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
        .map_err(|e| format!("SetWindowPos failed: {}", e))?;
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn configure_overlay(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[command]
async fn show_overlay(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    info!("show_overlay called");
    if let Some(window) = app.get_webview_window("overlay") {
        info!("show_overlay: overlay window found");
        configure_overlay(&window)?;

        // Position in bottom right of screen
        if let Ok(Some(monitor)) = window.current_monitor() {
            let screen_size = monitor.size();
            let screen_pos = monitor.position();
            if let Ok(window_size) = window.outer_size() {
                let x = screen_pos.x + (screen_size.width as i32) - (window_size.width as i32);
                let y = screen_pos.y + (screen_size.height as i32) - (window_size.height as i32);
                let _ = window
                    .set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
                info!("show_overlay: positioned at ({}, {})", x, y);
            }
        }

        window.show().map_err(|e| e.to_string())?;
        info!("show_overlay: window.show() completed");
        window.set_focus().map_err(|e| e.to_string())?;

        // Update state
        *state.overlay_visible.lock().unwrap() = true;

        // Update tray menu text
        if let Some(menu_item) = state.toggle_menu_item.lock().unwrap().as_ref() {
            let _ = menu_item.set_text("Hide Character");
        }

        // Emit event
        let _ = app.emit("overlay-visibility-changed", json!({ "visible": true }));
        info!("show_overlay: completed successfully");
    } else {
        info!("show_overlay: overlay window NOT found");
    }
    Ok(())
}

#[command]
async fn hide_overlay(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        window.hide().map_err(|e| e.to_string())?;

        // Update state
        *state.overlay_visible.lock().unwrap() = false;

        // Update tray menu text
        if let Some(menu_item) = state.toggle_menu_item.lock().unwrap().as_ref() {
            let _ = menu_item.set_text("Show Character");
        }

        // Emit event
        let _ = app.emit("overlay-visibility-changed", json!({ "visible": false }));
    }
    Ok(())
}

#[command]
async fn toggle_overlay(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let is_visible = *state.overlay_visible.lock().unwrap();

    if is_visible {
        hide_overlay(app, state).await?;
        Ok(false)
    } else {
        show_overlay(app, state).await?;
        Ok(true)
    }
}

// Sync version for use in tray handlers (non-async context)
fn toggle_overlay_sync(app: &AppHandle) {
    let state = app.state::<AppState>();
    let is_visible = *state.overlay_visible.lock().unwrap();

    if is_visible {
        if let Some(window) = app.get_webview_window("overlay") {
            let _ = window.hide();
            *state.overlay_visible.lock().unwrap() = false;
            let _ = app.emit("overlay-visibility-changed", json!({ "visible": false }));

            // Update tray menu text
            if let Some(menu_item) = state.toggle_menu_item.lock().unwrap().as_ref() {
                let _ = menu_item.set_text("Show Character");
            }
        }
    } else if let Some(window) = app.get_webview_window("overlay") {
        let _ = configure_overlay(&window);

        // Position in bottom right of screen
        if let Ok(Some(monitor)) = window.current_monitor() {
            let screen_size = monitor.size();
            let screen_pos = monitor.position();
            if let Ok(window_size) = window.outer_size() {
                let x = screen_pos.x + (screen_size.width as i32) - (window_size.width as i32);
                let y = screen_pos.y + (screen_size.height as i32) - (window_size.height as i32);
                let _ = window
                    .set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
            }
        }

        let _ = window.show();
        let _ = window.set_focus();
        *state.overlay_visible.lock().unwrap() = true;
        let _ = app.emit("overlay-visibility-changed", json!({ "visible": true }));

        // Update tray menu text
        if let Some(menu_item) = state.toggle_menu_item.lock().unwrap().as_ref() {
            let _ = menu_item.set_text("Hide Character");
        }
    }
}

#[command]
async fn get_overlay_visible(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    Ok(*state.overlay_visible.lock().unwrap())
}

/// Load saved overlay scale (returns 1.0 if not saved)
fn load_overlay_scale() -> f64 {
    if let Ok(path) = paths::get_overlay_scale_path() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(scale) = content.trim().parse::<f64>() {
                return scale.clamp(0.5, 2.0);
            }
        }
    }
    1.0
}

/// Save overlay scale to file
fn save_overlay_scale_to_file(scale: f64) -> Result<(), String> {
    let path = paths::get_overlay_scale_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    std::fs::write(&path, scale.to_string())
        .map_err(|e| format!("Failed to save overlay scale: {}", e))
}

#[command]
async fn resize_overlay(app: AppHandle, scale: f64) -> Result<(), String> {
    let scale = scale.clamp(0.5, 2.0);

    if let Some(window) = app.get_webview_window("overlay") {
        let width = paths::DEFAULT_OVERLAY_WIDTH * scale;
        let height = paths::DEFAULT_OVERLAY_HEIGHT * scale;

        // Resize the window using logical size (works correctly on Retina displays)
        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }))
            .map_err(|e| format!("Failed to resize overlay: {}", e))?;

        // Reposition to bottom right
        if let Ok(Some(monitor)) = window.current_monitor() {
            let scale_factor = monitor.scale_factor();
            let screen_size = monitor.size();
            let screen_pos = monitor.position();
            // Convert logical width/height to physical for position calculation
            let physical_width = (width * scale_factor) as i32;
            let physical_height = (height * scale_factor) as i32;
            let x = screen_pos.x + (screen_size.width as i32) - physical_width;
            let y = screen_pos.y + (screen_size.height as i32) - physical_height;
            let _ =
                window.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
        }

        // Save the scale
        save_overlay_scale_to_file(scale)?;
    }

    Ok(())
}

#[command]
async fn get_overlay_scale() -> Result<f64, String> {
    Ok(load_overlay_scale())
}

#[command]
async fn hide_main_window(app: AppHandle) -> Result<(), String> {
    info!("[hide_main_window] Attempting to hide main window");
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| {
            error!("[hide_main_window] Failed to hide window: {}", e);
            e.to_string()
        })?;
        info!("[hide_main_window] Window hidden, emitting event");
        let _ = app.emit(
            "main-window-visibility-changed",
            json!({ "visible": false }),
        );
        info!("[hide_main_window] Event emitted successfully");
    } else {
        warn!("[hide_main_window] Main window not found");
    }
    Ok(())
}

#[command]
async fn show_main_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        let _ = app.emit("main-window-visibility-changed", json!({ "visible": true }));
    }
    Ok(())
}

#[command]
async fn toggle_main_window(app: AppHandle) -> Result<bool, String> {
    if let Some(window) = app.get_webview_window("main") {
        let is_visible = window.is_visible().map_err(|e| e.to_string())?;
        if is_visible {
            window.hide().map_err(|e| e.to_string())?;
            let _ = app.emit(
                "main-window-visibility-changed",
                json!({ "visible": false }),
            );
            Ok(false)
        } else {
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
            let _ = app.emit("main-window-visibility-changed", json!({ "visible": true }));
            Ok(true)
        }
    } else {
        Ok(false)
    }
}

#[command]
async fn is_main_window_visible(app: AppHandle) -> Result<bool, String> {
    if let Some(window) = app.get_webview_window("main") {
        window.is_visible().map_err(|e| e.to_string())
    } else {
        Ok(false)
    }
}

// ============ Device Listening ============

#[derive(Debug, Clone, Serialize)]
pub struct DeviceEvent {
    kind: String,
    value: Value,
}

static IS_LISTENING: AtomicBool = AtomicBool::new(false);

#[command]
async fn start_device_listening(app: AppHandle) -> Result<(), String> {
    if IS_LISTENING.load(Ordering::SeqCst) {
        return Ok(());
    }
    IS_LISTENING.store(true, Ordering::SeqCst);

    std::thread::spawn(move || {
        let callback = move |event: Event| {
            // Mouse tracking for head movement
            if let EventType::MouseMove { x, y } = event.event_type {
                let device_event = DeviceEvent {
                    kind: "MouseMove".to_string(),
                    value: json!({ "x": x, "y": y }),
                };
                let _ = app.emit("device-changed", device_event);
            }
        };
        listen(callback).ok();
    });

    Ok(())
}

// ============ Screenshot ============

// Native macOS screen capture permission APIs
#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

#[command]
async fn check_screen_permission() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        unsafe {
            // First check if we already have permission
            if CGPreflightScreenCaptureAccess() {
                return Ok(true);
            }
            // If not, request permission (triggers system dialog)
            Ok(CGRequestScreenCaptureAccess())
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

#[command]
async fn open_screen_recording_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .spawn()
            .map_err(|e| format!("Failed to open settings: {}", e))?;
    }
    Ok(())
}

#[command]
async fn take_screenshot(app: AppHandle) -> Result<String, String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Generate filename with timestamp hash
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("Time error: {}", e))?
        .as_millis();
    let filename = format!("{:x}.png", timestamp);

    // Get screenshots directory and create if needed
    let screenshots_dir = get_screenshots_dir()?;
    std::fs::create_dir_all(&screenshots_dir)
        .map_err(|e| format!("Failed to create screenshots directory: {}", e))?;

    let filepath = screenshots_dir.join(&filename);

    // Use native screencapture on macOS (fast, captures all windows like cmd+shift+4)
    #[cfg(target_os = "macos")]
    {
        // Get display index from overlay window (for multi-monitor support)
        let display_index = if let Some(window) = app.get_webview_window("overlay") {
            if let Ok(Some(monitor)) = window.current_monitor() {
                if let Ok(monitors) = window.available_monitors() {
                    monitors
                        .iter()
                        .position(|m| m.name() == monitor.name())
                        .map(|i| i + 1)
                        .unwrap_or(1)
                } else {
                    1
                }
            } else {
                1
            }
        } else {
            1
        };

        let output = std::process::Command::new("screencapture")
            .arg("-x") // no sound
            .arg("-D")
            .arg(display_index.to_string())
            .arg(&filepath)
            .output()
            .map_err(|e| format!("Failed to run screencapture: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("could not create image") {
                return Err("Screen recording permission required. Go to System Settings > Privacy & Security > Screen Recording and enable Oto Desktop.".to_string());
            }
            return Err(format!("screencapture failed: {}", stderr));
        }
    }

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Graphics::Gdi::*;
        use windows::Win32::UI::WindowsAndMessaging::*;

        // Get monitor bounds from overlay window
        let (left, top, width, height) = if let Some(window) = app.get_webview_window("overlay") {
            if let Ok(Some(monitor)) = window.current_monitor() {
                let pos = monitor.position();
                let size = monitor.size();
                (pos.x, pos.y, size.width as i32, size.height as i32)
            } else {
                // Fallback to primary screen
                unsafe {
                    (
                        0,
                        0,
                        GetSystemMetrics(SM_CXSCREEN),
                        GetSystemMetrics(SM_CYSCREEN),
                    )
                }
            }
        } else {
            // Fallback to primary screen
            unsafe {
                (
                    0,
                    0,
                    GetSystemMetrics(SM_CXSCREEN),
                    GetSystemMetrics(SM_CYSCREEN),
                )
            }
        };

        unsafe {
            // Get desktop DC
            let screen_dc = GetDC(None);
            if screen_dc.is_invalid() {
                return Err("Failed to get screen DC".to_string());
            }

            // Create compatible DC and bitmap
            let mem_dc = CreateCompatibleDC(screen_dc);
            if mem_dc.is_invalid() {
                ReleaseDC(None, screen_dc);
                return Err("Failed to create compatible DC".to_string());
            }

            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
            if bitmap.is_invalid() {
                let _ = DeleteDC(mem_dc);
                ReleaseDC(None, screen_dc);
                return Err("Failed to create bitmap".to_string());
            }

            // Select bitmap into DC and copy screen from the correct monitor
            let old_bitmap = SelectObject(mem_dc, bitmap);
            BitBlt(mem_dc, 0, 0, width, height, screen_dc, left, top, SRCCOPY)
                .map_err(|e| format!("BitBlt failed: {}", e))?;

            // Prepare bitmap info for GetDIBits
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // Negative for top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default()],
            };

            // Allocate buffer and get pixels
            let mut pixels: Vec<u8> = vec![0; (width * height * 4) as usize];
            GetDIBits(
                mem_dc,
                bitmap,
                0,
                height as u32,
                Some(pixels.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            // Cleanup GDI objects
            SelectObject(mem_dc, old_bitmap);
            let _ = DeleteObject(bitmap);
            let _ = DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);

            // Convert BGRA to RGBA
            for chunk in pixels.chunks_exact_mut(4) {
                chunk.swap(0, 2); // Swap B and R
            }

            // Save using image crate
            let img = image::RgbaImage::from_raw(width as u32, height as u32, pixels)
                .ok_or("Failed to create image from pixels")?;
            img.save(&filepath)
                .map_err(|e| format!("Failed to save screenshot: {}", e))?;
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Check if running in WSL
        let is_wsl = std::fs::read_to_string("/proc/version")
            .map(|v| v.to_lowercase().contains("microsoft") || v.to_lowercase().contains("wsl"))
            .unwrap_or(false);

        if is_wsl {
            // In WSL, use PowerShell to capture Windows desktop
            // Save to Windows temp first, then copy to WSL location
            let temp_filename = format!("oto_screenshot_{}.png", std::process::id());
            let ps_script = format!(
                "Add-Type -AssemblyName System.Windows.Forms; \
                 $screen = [System.Windows.Forms.Screen]::PrimaryScreen; \
                 $bitmap = New-Object System.Drawing.Bitmap($screen.Bounds.Width, $screen.Bounds.Height); \
                 $graphics = [System.Drawing.Graphics]::FromImage($bitmap); \
                 $graphics.CopyFromScreen($screen.Bounds.Location, [System.Drawing.Point]::Empty, $screen.Bounds.Size); \
                 $bitmap.Save(\"$env:TEMP\\\\{}\");",
                temp_filename
            );
            let output = std::process::Command::new("powershell.exe")
                .args(["-Command", &ps_script])
                .output()
                .map_err(|e| format!("Failed to capture screenshot via PowerShell: {}", e))?;

            if !output.status.success() {
                return Err(format!(
                    "PowerShell screenshot failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }

            // Get Windows username and copy from Windows temp to WSL location
            let win_user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
            let temp_path = format!(
                "/mnt/c/Users/{}/AppData/Local/Temp/{}",
                win_user, temp_filename
            );

            // Copy from Windows temp to final location
            std::fs::copy(&temp_path, &filepath).map_err(|e| {
                format!(
                    "Failed to copy screenshot from temp: {} (temp: {})",
                    e, temp_path
                )
            })?;

            // Clean up temp file
            let _ = std::fs::remove_file(&temp_path);
        } else {
            // Native Linux: use gnome-screenshot or scrot
            let output = std::process::Command::new("gnome-screenshot")
                .arg("-f")
                .arg(&filepath)
                .output();

            if output.is_err() || !output.as_ref().unwrap().status.success() {
                std::process::Command::new("scrot")
                    .arg(&filepath)
                    .output()
                    .map_err(|e| {
                        format!(
                            "Failed to capture screenshot (install gnome-screenshot or scrot): {}",
                            e
                        )
                    })?;
            }
        }
    }

    println!("[screenshot] Saved to: {:?}", filepath);

    Ok(filepath.to_string_lossy().to_string())
}

#[command]
async fn open_screenshots_folder() -> Result<(), String> {
    let screenshots_dir = get_screenshots_dir()?;

    // Create directory if it doesn't exist
    std::fs::create_dir_all(&screenshots_dir)
        .map_err(|e| format!("Failed to create screenshots directory: {}", e))?;

    // Open in file manager
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&screenshots_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&screenshots_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&screenshots_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    Ok(())
}

// ============ Main ============

fn main() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();

    // Only register localfile:// protocol on Windows where Tauri's convertFileSrc is broken
    #[cfg(target_os = "windows")]
    {
        builder = builder.register_asynchronous_uri_scheme_protocol(
            "localfile",
            |_ctx, request, responder| {
                std::thread::spawn(move || {
                    let uri = request.uri();
                    let path_str = uri.path();

                    // URL decode the path
                    let decoded =
                        urlencoding::decode(path_str).unwrap_or_else(|_| path_str.into());

                    // Strip query string if present (e.g., ?t=123456 cache buster)
                    let path_without_query = decoded.split('?').next().unwrap_or(&decoded);

                    // On Windows, the path comes as /C:/Users/... so we need to strip the leading /
                    let file_path = if path_without_query.starts_with('/')
                        && path_without_query.chars().nth(2) == Some(':')
                    {
                        path_without_query[1..].to_string()
                    } else {
                        path_without_query.to_string()
                    };

                    match std::fs::read(&file_path) {
                        Ok(content) => {
                            let mime = mime_guess::from_path(&file_path)
                                .first_or_octet_stream()
                                .to_string();

                            let response = Response::builder()
                                .header("Content-Type", &mime)
                                .header("Access-Control-Allow-Origin", "*")
                                .body(content)
                                .unwrap();

                            responder.respond(response);
                        }
                        Err(e) => {
                            error!("[localfile] Failed to read file {}: {}", file_path, e);
                            let response = Response::builder()
                                .status(404)
                                .header("Content-Type", "text/plain")
                                .body(format!("File not found: {}", e).into_bytes())
                                .unwrap();
                            responder.respond(response);
                        }
                    }
                });
            },
        );
    }

    builder
        .manage(AppState::default())
        .setup(|app| {
            // Log startup information
            info!("=== OTO Desktop Starting ===");
            if let Ok(models_dir) = get_models_dir() {
                info!("[startup] Models directory: {:?}", models_dir);
                info!("[startup] Models directory exists: {}", models_dir.exists());
            }
            if let Ok(config) = load_model_config() {
                info!(
                    "[startup] Current model config - folder: {}, model_file: {}",
                    config.folder, config.model_file
                );
            }

            // Create tray menu
            let toggle_item =
                MenuItem::with_id(app, "toggle", "Show Character", true, None::<&str>)?;
            let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            // Store toggle item in state for later text updates
            let state = app.state::<AppState>();
            *state.toggle_menu_item.lock().unwrap() = Some(toggle_item.clone());

            let menu = Menu::with_items(app, &[&toggle_item, &settings_item, &quit_item])?;

            // Create tray icon
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |_app, event| {
                    match event.id.as_ref() {
                        "toggle" => {
                            toggle_overlay_sync(_app);
                        }
                        "chat_history" => {
                            // Show overlay and emit event to open history modal
                            if let Some(window) = _app.get_webview_window("overlay") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                            let _ = _app.emit("show-chat-history", ());
                        }
                        "settings" => {
                            // Show main window (for API key entry, etc.)
                            if let Some(window) = _app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                                let _ = _app.emit(
                                    "main-window-visibility-changed",
                                    json!({ "visible": true }),
                                );
                            }
                        }
                        "screenshots" => {
                            // Open screenshots folder
                            std::thread::spawn(|| {
                                let _ = tauri::async_runtime::block_on(open_screenshots_folder());
                            });
                        }
                        "clear_data" => {
                            if let Err(e) = clear_app_data() {
                                error!("Error clearing app data: {}", e);
                            }
                        }
                        "quit" => {
                            std::process::exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_overlay_sync(tray.app_handle());
                    }
                })
                .build(app)?;

            // Register global shortcut (Option+Space on macOS, Super+Space on Linux/WSL, Alt+Space on Windows)
            #[cfg(target_os = "linux")]
            let shortcut = Shortcut::new(Some(Modifiers::SUPER), Code::Space);
            #[cfg(not(target_os = "linux"))]
            let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Space);
            app.global_shortcut().register(shortcut)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    // Prevent the window from actually closing - just hide it
                    api.prevent_close();
                    let _ = window.hide();
                    let _ = window.app_handle().emit(
                        "main-window-visibility-changed",
                        serde_json::json!({ "visible": false }),
                    );
                }
            }
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("oto.log".into()),
                    },
                ))
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed
                        && (shortcut.matches(Modifiers::ALT, Code::Space)
                            || shortcut.matches(Modifiers::SUPER, Code::Space))
                    {
                        // Show overlay if hidden
                        let is_visible = {
                            let state = app.state::<AppState>();
                            let visible = *state.overlay_visible.lock().unwrap();
                            visible
                        };
                        if !is_visible {
                            toggle_overlay_sync(app);
                        }
                        // Focus the overlay window so keyboard input works
                        if let Some(window) = app.get_webview_window("overlay") {
                            let _ = window.set_focus();
                        }
                        let _ = app.emit("toggle-textbox", ());
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            init_app,
            get_paths,
            read_file_as_text,
            read_file_as_bytes,
            is_initialized,
            get_model_config,
            change_model,
            reset_model,
            load_model_from_folder,
            show_overlay,
            hide_overlay,
            toggle_overlay,
            get_overlay_visible,
            resize_overlay,
            get_overlay_scale,
            hide_main_window,
            show_main_window,
            toggle_main_window,
            is_main_window_visible,
            start_device_listening,
            check_screen_permission,
            open_screen_recording_settings,
            take_screenshot,
            open_screenshots_folder,
            save_api_key,
            get_api_key,
            has_api_key,
            save_system_prompt,
            get_system_prompt,
            save_character_prompt,
            get_character_prompt,
            save_deep_research_prompt,
            get_deep_research_prompt,
            save_dialogue_prompt,
            get_dialogue_prompt,
            send_chat_message,
            get_chat_history,
            clear_chat_history,
            trigger_deep_research,
            clear_all_data,
            generate_texture,
            get_texture_paths,
            reload_character,
            get_texture_versions,
            apply_texture_version,
            delete_texture_version,
            save_hitbox,
            load_hitbox,
            clear_hitbox,
            save_transform_config,
            load_transform_config,
            log_from_frontend,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
