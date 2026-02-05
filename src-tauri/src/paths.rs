//! Path utilities and file system helpers

use std::path::PathBuf;

/// URL for downloading the default Live2D model
pub const DEFAULT_MODEL_URL: &str = "https://storage.googleapis.com/oto_bucket/live2d/Hiyori1.zip";

/// Default overlay window dimensions
pub const DEFAULT_OVERLAY_WIDTH: f64 = 400.0;
pub const DEFAULT_OVERLAY_HEIGHT: f64 = 600.0;

/// Gets the application data directory
pub fn get_app_data_dir() -> Result<PathBuf, String> {
    dirs::data_dir()
        .map(|p| p.join("com.oto.desktop"))
        .ok_or_else(|| "Could not find app data directory".to_string())
}

/// Clears all application data
pub fn clear_app_data() -> Result<(), String> {
    let app_dir = get_app_data_dir()?;
    if app_dir.exists() {
        std::fs::remove_dir_all(&app_dir)
            .map_err(|e| format!("Failed to clear app data: {}", e))?;
    }
    Ok(())
}

/// Gets the models directory path
pub fn get_models_dir() -> Result<PathBuf, String> {
    get_app_data_dir().map(|p| p.join("models"))
}

/// Gets the screenshots directory path
pub fn get_screenshots_dir() -> Result<PathBuf, String> {
    get_app_data_dir().map(|p| p.join("History").join("Screenshots"))
}

/// Gets the database file path
pub fn get_db_path() -> Result<PathBuf, String> {
    get_app_data_dir().map(|p| p.join("chat_history.db"))
}

/// Gets the API key file path
pub fn get_api_key_path() -> Result<PathBuf, String> {
    get_app_data_dir().map(|p| p.join(".api_key"))
}

/// Gets the system prompt file path
pub fn get_system_prompt_path() -> Result<PathBuf, String> {
    get_app_data_dir().map(|p| p.join(".system_prompt"))
}

/// Gets the character prompt file path
pub fn get_character_prompt_path() -> Result<PathBuf, String> {
    get_app_data_dir().map(|p| p.join(".character_prompt"))
}

/// Gets the dialogue prompt file path
pub fn get_dialogue_prompt_path() -> Result<PathBuf, String> {
    get_app_data_dir().map(|p| p.join(".dialogue_prompt"))
}

/// Gets the hitbox configuration file path
pub fn get_hitbox_path() -> Result<PathBuf, String> {
    get_app_data_dir().map(|p| p.join(".hitbox.json"))
}

/// Gets the model configuration file path
pub fn get_model_config_path() -> Result<PathBuf, String> {
    get_app_data_dir().map(|p| p.join(".model_config.json"))
}

/// Gets the overlay scale file path
pub fn get_overlay_scale_path() -> Result<PathBuf, String> {
    get_app_data_dir().map(|p| p.join(".overlay_scale"))
}

/// Gets the transform configuration file path
pub fn get_transform_config_path() -> Result<PathBuf, String> {
    get_app_data_dir().map(|p| p.join(".transform_config.json"))
}

/// Gets the Codex code generation workspace directory
pub fn get_codex_workspace_dir() -> Result<PathBuf, String> {
    get_app_data_dir().map(|p| p.join("codex_workspace"))
}

/// Gets the texture directory path for a specific model
pub fn get_texture_dir_for_model(
    model_folder: &str,
    texture_folder: &str,
) -> Result<PathBuf, String> {
    get_models_dir().map(|p| p.join(model_folder).join(texture_folder))
}

/// Gets the originals backup directory path for a specific model
pub fn get_originals_dir_for_model(
    model_folder: &str,
    texture_folder: &str,
) -> Result<PathBuf, String> {
    get_texture_dir_for_model(model_folder, texture_folder).map(|p| p.join("originals"))
}

/// Gets the texture versions directory path for a specific model
pub fn get_versions_dir_for_model(
    model_folder: &str,
    texture_folder: &str,
) -> Result<PathBuf, String> {
    get_texture_dir_for_model(model_folder, texture_folder).map(|p| p.join("versions"))
}
