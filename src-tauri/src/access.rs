//! Where agents may read and write files through the MCP server. Every path an agent passes (an
//! image or SVG file, an assets folder, a screenshot, a project to scan) is checked against the
//! folders the user allowed; the first time an agent reaches a new folder, Pergel asks. The answer
//! holds until the app quits.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{AppHandle, Manager};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    /// Reading one file (permission covers its folder).
    Read,
    /// Writing into a folder (created if needed).
    Write,
    /// Reading the source code under a folder.
    Scan,
}

pub struct Access {
    app: AppHandle,
    allowed: Mutex<Vec<PathBuf>>,
    /// One question at a time.
    asking: tokio::sync::Mutex<()>,
}

/// Windows' canonical paths start with `\\?\`; the plain form names the same place and reads
/// better in messages.
fn plain(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path,
    }
}

/// `path` without `.`, `..` or links: the existing part resolved on disk, then the part that
/// doesn't exist yet (a folder to create) appended as written.
pub fn resolve(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("{} is not an absolute path; pass the full path", path.display()));
    }
    let mut existing = path.to_path_buf();
    let mut missing = Vec::new();
    while !existing.exists() {
        // `file_name` is None for a path ending in `..`, which can't be checked before it exists.
        let Some(name) = existing.file_name() else { return Err(format!("{} is not a usable path", path.display())) };
        missing.push(name.to_os_string());
        if !existing.pop() {
            return Err(format!("{} is not a usable path", path.display()));
        }
    }
    let mut out = plain(std::fs::canonicalize(&existing).map_err(|e| format!("Could not resolve {}: {e}", existing.display()))?);
    for name in missing.iter().rev() {
        if name == "." || name == ".." {
            return Err(format!("{} is not a usable path", path.display()));
        }
        out.push(name);
    }
    Ok(out)
}

/// Whether `folder` is one of `roots` or inside one.
fn inside(folder: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|r| folder.starts_with(r))
}

impl Access {
    pub fn new(app: AppHandle) -> Self {
        Self { app, allowed: Mutex::new(Vec::new()), asking: tokio::sync::Mutex::new(()) }
    }

    fn allows(&self, folder: &Path) -> bool {
        self.allowed.lock().is_ok_and(|roots| inside(folder, &roots))
    }

    /// The resolved `path` when the agent may use it this way; asks the user about a new folder.
    pub async fn check(&self, path: &Path, how: Use) -> Result<PathBuf, String> {
        let target = resolve(path)?;
        let folder = match how {
            Use::Read => target.parent().map_or_else(|| target.clone(), Path::to_path_buf),
            Use::Write | Use::Scan => target.clone(),
        };
        if self.allows(&folder) {
            return Ok(target);
        }
        let _turn = self.asking.lock().await;
        // Allowed while this call waited for another question.
        if self.allows(&folder) {
            return Ok(target);
        }
        let shown = folder.display().to_string();
        let verb = match how {
            Use::Read => "read files in",
            Use::Write => "write files to",
            Use::Scan => "read the source code in",
        };
        let app = self.app.clone();
        let message = format!("An AI agent connected to Pergel wants to {verb}\n\n{shown}\n\nAllow it until Pergel quits?");
        let yes = tauri::async_runtime::spawn_blocking(move || {
            use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
            let mut dialog = app
                .dialog()
                .message(message)
                .title("Allow folder access?")
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom("Allow".into(), "Don't allow".into()));
            if let Some(window) = app.get_webview_window("main") {
                dialog = dialog.parent(&window);
            }
            dialog.blocking_show()
        })
        .await
        .unwrap_or(false);
        if !yes {
            return Err(format!("The user did not allow access to {shown}"));
        }
        self.allowed.lock().map_err(|e| e.to_string())?.push(folder);
        Ok(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_resolve_without_dots_and_must_be_absolute() {
        let tmp = plain(std::fs::canonicalize(std::env::temp_dir()).unwrap());
        assert_eq!(resolve(&tmp.join("pergel-new").join("assets")).unwrap(), tmp.join("pergel-new").join("assets"));
        // `..` in the existing part is resolved away; in the part still to be created it is refused.
        let parent = tmp.parent().unwrap().to_path_buf();
        assert_eq!(resolve(&tmp.join("..")).unwrap(), parent);
        // Whatever a path with `..` resolves to, the result (what is checked and used) has none.
        if let Ok(p) = resolve(&tmp.join("pergel-new").join("..").join("..").join("x")) {
            assert!(p.components().all(|c| !matches!(c, std::path::Component::ParentDir | std::path::Component::CurDir)), "{p:?}");
        }
        assert!(resolve(Path::new("relative/assets")).is_err());
    }

    #[test]
    fn a_folder_inside_an_allowed_one_is_allowed() {
        let roots = vec![PathBuf::from("C:/app/public")];
        assert!(inside(Path::new("C:/app/public"), &roots));
        assert!(inside(Path::new("C:/app/public/assets/icons"), &roots));
        assert!(!inside(Path::new("C:/app/publicity"), &roots));
        assert!(!inside(Path::new("C:/app"), &roots));
    }
}
