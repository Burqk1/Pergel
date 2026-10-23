//! Local file storage, like Figma's Drafts: each document is `<id>.pergel` (JSON) plus an
//! `<id>.png` thumbnail in the app data folder, listed in `index.json`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub id: String,
    pub name: String,
    /// Milliseconds since the Unix epoch.
    pub created: u64,
    pub modified: u64,
    pub opened: u64,
    #[serde(default)]
    pub starred: bool,
    #[serde(default)]
    pub trashed: bool,
}

#[derive(Default)]
pub struct Store {
    lock: Mutex<()>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("files");
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    Ok(dir)
}

fn read_index(dir: &Path) -> Vec<FileMeta> {
    fs::read_to_string(dir.join("index.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn write_index(dir: &Path, files: &[FileMeta]) -> Result<(), String> {
    let text = serde_json::to_string_pretty(files).map_err(|e| e.to_string())?;
    // Write then rename, so a crash never leaves a half-written index.
    let tmp = dir.join("index.json.tmp");
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, dir.join("index.json")).map_err(|e| e.to_string())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    format!("{:x}{:04x}", now(), COUNTER.fetch_add(1, Ordering::Relaxed) & 0xffff)
}

fn with_index<T>(
    app: &AppHandle,
    store: &Store,
    f: impl FnOnce(&Path, &mut Vec<FileMeta>) -> Result<T, String>,
) -> Result<T, String> {
    let _guard = store.lock.lock().map_err(|e| e.to_string())?;
    let dir = dir(app)?;
    let mut files = read_index(&dir);
    let out = f(&dir, &mut files)?;
    write_index(&dir, &files)?;
    Ok(out)
}

fn find<'a>(files: &'a mut [FileMeta], id: &str) -> Result<&'a mut FileMeta, String> {
    files.iter_mut().find(|f| f.id == id).ok_or_else(|| format!("No file {id}"))
}

#[tauri::command]
pub async fn files_list(app: AppHandle, store: State<'_, Store>) -> Result<Vec<FileMeta>, String> {
    with_index(&app, &store, |_, files| Ok(files.clone()))
}

#[tauri::command]
pub async fn file_create(app: AppHandle, store: State<'_, Store>, name: String, content: String) -> Result<FileMeta, String> {
    with_index(&app, &store, |dir, files| {
        let t = now();
        let meta = FileMeta { id: new_id(), name, created: t, modified: t, opened: t, starred: false, trashed: false };
        write_atomic(&dir.join(format!("{}.pergel", meta.id)), content.as_bytes())?;
        files.push(meta.clone());
        Ok(meta)
    })
}

#[tauri::command]
pub async fn file_read(app: AppHandle, store: State<'_, Store>, id: String) -> Result<String, String> {
    with_index(&app, &store, |dir, files| {
        find(files, &id)?.opened = now();
        fs::read_to_string(dir.join(format!("{id}.pergel"))).map_err(|e| format!("Could not open the file: {e}"))
    })
}

#[tauri::command]
pub async fn file_save(
    app: AppHandle,
    store: State<'_, Store>,
    id: String,
    content: String,
    thumbnail: Option<String>,
) -> Result<FileMeta, String> {
    with_index(&app, &store, |dir, files| {
        let path = dir.join(format!("{id}.pergel"));
        // Autosave history: keep the previous content when the last version is old enough.
        if let Ok(previous) = fs::read(&path) {
            let latest = list_versions(dir, &id).first().map_or(0, |v| v.time);
            if now().saturating_sub(latest) > AUTO_VERSION_MS && previous != content.as_bytes() {
                let _ = save_version(dir, &id, &previous, "");
            }
        }
        write_atomic(&path, content.as_bytes())?;
        if let Some(png) = thumbnail.and_then(|b| B64.decode(b).ok()) {
            write_atomic(&dir.join(format!("{id}.png")), &png)?;
        }
        let meta = find(files, &id)?;
        meta.modified = now();
        Ok(meta.clone())
    })
}

/// Autosaved versions are kept at most this often (10 minutes), the newest MAX_VERSIONS of them.
const AUTO_VERSION_MS: u64 = 10 * 60 * 1000;
const MAX_VERSIONS: usize = 60;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    /// Milliseconds since the Unix epoch; also the version's key.
    pub time: u64,
    /// Name given with "Save to version history", empty for autosaves.
    pub label: String,
    pub size: u64,
}

fn versions_dir(dir: &Path, id: &str) -> PathBuf {
    dir.join("versions").join(id)
}

/// Newest first.
fn list_versions(dir: &Path, id: &str) -> Vec<Version> {
    let mut out: Vec<Version> = fs::read_dir(versions_dir(dir, id))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let stem = name.strip_suffix(".pergel")?;
            let (time, label) = stem.split_once('_').unwrap_or((stem, ""));
            let label = label.replace('~', " ");
            Some(Version { time: time.parse().ok()?, label, size: e.metadata().map_or(0, |m| m.len()) })
        })
        .collect();
    out.sort_by(|a, b| b.time.cmp(&a.time));
    out
}

fn save_version(dir: &Path, id: &str, content: &[u8], label: &str) -> Result<Version, String> {
    let vdir = versions_dir(dir, id);
    fs::create_dir_all(&vdir).map_err(|e| e.to_string())?;
    let clean: String = label.chars().map(|c| if c.is_alphanumeric() || c == '-' { c } else { '~' }).take(60).collect();
    let time = now();
    let name = if clean.is_empty() { format!("{time}.pergel") } else { format!("{time}_{clean}.pergel") };
    write_atomic(&vdir.join(name), content)?;
    // Keep named versions; drop the oldest autosaves beyond the limit.
    let all = list_versions(dir, id);
    for old in all.iter().filter(|v| v.label.is_empty()).skip(MAX_VERSIONS) {
        let _ = fs::remove_file(vdir.join(format!("{}.pergel", old.time)));
    }
    Ok(Version { time, label: label.to_string(), size: content.len() as u64 })
}

fn version_path(dir: &Path, id: &str, time: u64) -> Option<PathBuf> {
    fs::read_dir(versions_dir(dir, id)).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.file_stem().and_then(|s| s.to_str()).is_some_and(|s| s == time.to_string() || s.starts_with(&format!("{time}_")))
    })
}

#[tauri::command]
pub async fn file_versions(app: AppHandle, id: String) -> Result<Vec<Version>, String> {
    Ok(list_versions(&dir(&app)?, &id))
}

/// "Save to version history" (Ctrl+Alt+S): the current content as a named version.
#[tauri::command]
pub async fn file_version_save(app: AppHandle, store: State<'_, Store>, id: String, content: String, label: String) -> Result<Version, String> {
    let _guard = store.lock.lock().map_err(|e| e.to_string())?;
    save_version(&dir(&app)?, &id, content.as_bytes(), label.trim())
}

#[tauri::command]
pub async fn file_version_read(app: AppHandle, id: String, time: u64) -> Result<String, String> {
    let path = version_path(&dir(&app)?, &id, time).ok_or("That version no longer exists")?;
    fs::read_to_string(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn file_thumbnail(app: AppHandle, id: String) -> Result<Option<String>, String> {
    let path = dir(&app)?.join(format!("{id}.png"));
    Ok(fs::read(path).ok().map(|b| B64.encode(b)))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub name: Option<String>,
    pub starred: Option<bool>,
    pub trashed: Option<bool>,
}

#[tauri::command]
pub async fn file_update(app: AppHandle, store: State<'_, Store>, id: String, change: FileChange) -> Result<FileMeta, String> {
    with_index(&app, &store, |_, files| {
        let meta = find(files, &id)?;
        if let Some(name) = change.name.filter(|n| !n.trim().is_empty()) {
            meta.name = name.trim().to_string();
        }
        if let Some(s) = change.starred {
            meta.starred = s;
        }
        if let Some(t) = change.trashed {
            meta.trashed = t;
        }
        Ok(meta.clone())
    })
}

#[tauri::command]
pub async fn file_duplicate(app: AppHandle, store: State<'_, Store>, id: String) -> Result<FileMeta, String> {
    with_index(&app, &store, |dir, files| {
        let src = find(files, &id)?.clone();
        let t = now();
        let copy = FileMeta { id: new_id(), name: format!("{} (Copy)", src.name), created: t, modified: t, opened: t, starred: false, trashed: false };
        fs::copy(dir.join(format!("{id}.pergel")), dir.join(format!("{}.pergel", copy.id))).map_err(|e| e.to_string())?;
        let _ = fs::copy(dir.join(format!("{id}.png")), dir.join(format!("{}.png", copy.id)));
        files.push(copy.clone());
        Ok(copy)
    })
}

/// Deletes a file for good (from the Trash).
#[tauri::command]
pub async fn file_delete(app: AppHandle, store: State<'_, Store>, id: String) -> Result<(), String> {
    with_index(&app, &store, |dir, files| {
        files.retain(|f| f.id != id);
        let _ = fs::remove_file(dir.join(format!("{id}.pergel")));
        let _ = fs::remove_file(dir.join(format!("{id}.png")));
        Ok(())
    })
}

/// Copies a `.pergel` file from disk into the store.
#[tauri::command]
pub async fn file_import(app: AppHandle, store: State<'_, Store>, path: String) -> Result<FileMeta, String> {
    let path = PathBuf::from(path);
    let content = fs::read_to_string(&path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    let name = path.file_stem().map_or("Imported".to_string(), |s| s.to_string_lossy().into_owned());
    file_create(app, store, name, content).await
}

/// Writes a stored file to a path on disk.
#[tauri::command]
pub async fn file_export(app: AppHandle, id: String, path: String) -> Result<(), String> {
    let src = dir(&app)?.join(format!("{id}.pergel"));
    fs::copy(src, &path).map(|_| ()).map_err(|e| format!("Could not write {path}: {e}"))
}

/// Writes arbitrary bytes (base64) to a path chosen by the user, for image/SVG/PDF exports.
#[tauri::command]
pub async fn write_file(path: String, base64: Option<String>, text: Option<String>) -> Result<(), String> {
    let bytes = match (base64, text) {
        (Some(b), _) => B64.decode(b).map_err(|e| e.to_string())?,
        (None, Some(t)) => t.into_bytes(),
        _ => return Err("Nothing to write".into()),
    };
    // Site publishing writes into folders that may not exist yet (assets/).
    if let Some(parent) = std::path::Path::new(&path).parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    fs::write(&path, bytes).map_err(|e| format!("Could not write {path}: {e}"))
}
