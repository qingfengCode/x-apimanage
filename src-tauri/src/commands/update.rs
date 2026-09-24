//! 应用自更新相关的 Tauri 命令，委托给 [`crate::updater`]。
//!
//! 更新源地址保存在应用数据目录的 `update.json`（与 sqlite 数据库同目录），
//! 格式：`{ "manifestUrl": "https://…/update.json" }`。

use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::updater::{self, UpdateManifest};

/// 关于弹窗展示用的应用信息。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// 当前运行版本。
    pub current_version: String,
    /// 更新清单地址（可能为空）。
    pub manifest_url: String,
    /// 应用数据目录（安装包下载位置）。
    pub data_dir: String,
}

/// 更新源配置文件。
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateConfig {
    #[serde(default)]
    manifest_url: String,
}

fn config_path(app: &AppHandle) -> AppResult<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .map(|d| d.join("update.json"))
        .map_err(|e| AppError::Other(format!("解析应用数据目录失败: {}", e)))
}

fn load_config(app: &AppHandle) -> AppResult<UpdateConfig> {
    let path = config_path(app)?;
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|e| AppError::Other(format!("解析 update.json 失败: {}", e))),
        Err(_) => Ok(UpdateConfig::default()), // 文件不存在视为未配置
    }
}

fn save_config(app: &AppHandle, cfg: &UpdateConfig) -> AppResult<()> {
    let path = config_path(app)?;
    let text = serde_json::to_string_pretty(cfg)
        .map_err(|e| AppError::Other(format!("序列化 update.json 失败: {}", e)))?;
    std::fs::write(&path, text)?;
    Ok(())
}

/// 当前版本号（取自 Cargo.toml 注入的 package info）。
fn current_version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

/// 返回当前版本 / 更新源 / 数据目录。
#[tauri::command]
pub fn update_get_info(app: AppHandle) -> AppResult<UpdateInfo> {
    let cfg = load_config(&app)?;
    let data_dir = app
        .path()
        .app_data_dir()
        .map(|d| d.display().to_string())
        .unwrap_or_default();
    Ok(UpdateInfo {
        current_version: current_version(&app),
        manifest_url: cfg.manifest_url,
        data_dir,
    })
}

/// 读取更新源地址。
#[tauri::command]
pub fn update_get_manifest_url(app: AppHandle) -> AppResult<String> {
    Ok(load_config(&app)?.manifest_url)
}

/// 保存更新源地址。
#[tauri::command]
pub fn update_set_manifest_url(app: AppHandle, url: String) -> AppResult<()> {
    let mut cfg = load_config(&app)?;
    cfg.manifest_url = url.trim().to_string();
    save_config(&app, &cfg)
}

/// 检查更新：返回可用清单，若已是最新返回 `None`（前端收到 null）。
#[tauri::command]
pub async fn update_check(app: AppHandle) -> AppResult<Option<UpdateManifest>> {
    let cfg = load_config(&app)?;
    let url = cfg.manifest_url.trim().to_string();
    if url.is_empty() {
        return Err(AppError::Update(
            "尚未配置更新源地址，请先在更新设置中填写".into(),
        ));
    }
    updater::check(&url, &current_version(&app)).await
}

/// 下载安装包，返回落地文件绝对路径。进度经 `update:progress` 事件推送。
#[tauri::command]
pub async fn update_download(
    app: AppHandle,
    manifest: UpdateManifest,
) -> AppResult<String> {
    let dest_dir = app
        .path()
        .app_data_dir()
        .map(|d| d.join("updates"))
        .map_err(|e| AppError::Other(format!("解析应用数据目录失败: {}", e)))?;
    let path = updater::download(&app, &manifest, &dest_dir).await?;
    Ok(path.display().to_string())
}

/// 拉起安装器并退出应用（不可逆）。
#[tauri::command]
pub fn update_install_and_exit(app: AppHandle, path: String) -> AppResult<()> {
    let installer = std::path::PathBuf::from(path);
    updater::install_and_exit(&app, &installer)
}
