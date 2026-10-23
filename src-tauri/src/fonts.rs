//! The computer's installed fonts, so designs (and imported Figma files) render with the fonts
//! they name. The list is read once; font files are only read when a family is used.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Serialize;

static DB: OnceLock<fontdb::Database> = OnceLock::new();

fn db() -> &'static fontdb::Database {
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    })
}

#[derive(Serialize, Clone)]
pub struct SystemFamily {
    pub name: String,
    pub weights: Vec<u16>,
    pub italic: bool,
}

fn family_of(face: &fontdb::FaceInfo) -> Option<&str> {
    // Prefer the English family name.
    face.families
        .iter()
        .find(|(_, lang)| *lang == fontdb::Language::English_UnitedStates)
        .or_else(|| face.families.first())
        .map(|(name, _)| name.as_str())
}

/// Every installed font family with its weights.
#[tauri::command]
pub async fn system_fonts() -> Vec<SystemFamily> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut map: BTreeMap<String, SystemFamily> = BTreeMap::new();
        for face in db().faces() {
            let Some(name) = family_of(face) else { continue };
            let entry = map
                .entry(name.to_lowercase())
                .or_insert_with(|| SystemFamily { name: name.to_string(), weights: Vec::new(), italic: false });
            if !entry.weights.contains(&face.weight.0) {
                entry.weights.push(face.weight.0);
            }
            entry.italic |= face.style != fontdb::Style::Normal;
        }
        let mut list: Vec<SystemFamily> = map.into_values().collect();
        for f in &mut list {
            f.weights.sort_unstable();
        }
        list
    })
    .await
    .unwrap_or_default()
}

/// The font files of one installed family, concatenated as `[u32 length][bytes]…`.
#[tauri::command]
pub async fn system_font_data(family: String) -> Result<tauri::ipc::Response, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for face in db().faces() {
            if !family_of(face).is_some_and(|n| n.eq_ignore_ascii_case(&family)) {
                continue;
            }
            let fontdb::Source::File(path) = &face.source else { continue };
            // A collection (.ttc) holds several faces; send each file once.
            if !seen.insert(path.clone()) {
                continue;
            }
            if let Ok(bytes) = std::fs::read(path) {
                out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                out.extend_from_slice(&bytes);
            }
        }
        if out.is_empty() { Err(format!("{family} is not installed")) } else { Ok(tauri::ipc::Response::new(out)) }
    })
    .await
    .map_err(|e| e.to_string())?
}
