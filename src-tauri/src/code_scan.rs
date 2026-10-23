//! Code Connect suggestions: finds the UI components of a codebase (React/Preact `.tsx`/`.jsx`,
//! Vue, Svelte) and matches them to the design's main components and component sets by name,
//! with variant → prop mappings when the code declares the same values.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

const SKIP: [&str; 12] = ["node_modules", ".git", "dist", "build", "target", ".next", "out", "coverage", ".svelte-kit", ".turbo", "vendor", "public"];
const MAX_FILES: usize = 6000;

#[derive(Debug, Clone)]
pub struct CodeComponent {
    pub name: String,
    /// Path relative to the project, with forward slashes.
    pub file: String,
    pub import: String,
    /// Props with the string literal values their type allows (`variant?: "default" | "ghost"`).
    pub props: BTreeMap<String, Vec<String>>,
}

fn files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                if !SKIP.contains(&name.as_str()) && !name.starts_with('.') {
                    stack.push(p);
                }
            } else if matches!(p.extension().and_then(|x| x.to_str()), Some("tsx" | "jsx" | "vue" | "svelte"))
                && !name.contains(".test.")
                && !name.contains(".spec.")
                && !name.contains(".stories.")
            {
                out.push(p);
                if out.len() >= MAX_FILES {
                    return out;
                }
            }
        }
    }
    out
}

fn pascal(stem: &str) -> String {
    stem.split(['-', '_', '.', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
        })
        .collect()
}

fn ident_at(s: &str) -> Option<&str> {
    let end = s.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(s.len());
    (end > 0).then(|| &s[..end])
}

/// Exported component names of a JSX/TSX module: capitalised functions and constants.
fn exports(src: &str) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = Vec::new();
    for (pat, default) in [
        ("export default function ", true),
        ("export function ", false),
        ("export const ", false),
        ("export default class ", true),
        ("export class ", false),
    ] {
        let mut from = 0;
        while let Some(at) = src[from..].find(pat) {
            let start = from + at + pat.len();
            if let Some(name) = ident_at(&src[start..])
                && name.chars().next().is_some_and(char::is_uppercase)
                && !out.iter().any(|o| o.0 == name)
            {
                out.push((name.to_string(), default));
            }
            from = start;
        }
    }
    // export { Button, buttonVariants }
    let mut from = 0;
    while let Some(at) = src[from..].find("export {") {
        let start = from + at + 8;
        let end = src[start..].find('}').map_or(src.len(), |e| start + e);
        for part in src[start..end].split(',') {
            let name = part.split(" as ").last().unwrap_or("").trim();
            if name.chars().next().is_some_and(char::is_uppercase) && !out.iter().any(|o| o.0 == name) {
                out.push((name.to_string(), false));
            }
        }
        from = end;
    }
    out
}

/// `name?: "a" | "b"` pairs inside the props type of a module (or a Vue/Svelte component), and
/// class-variance-authority `variants: { variant: { default: …, ghost: … } }`.
fn props(src: &str) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for piece in src.split(['\n', ';', '{', ',']) {
        let l = piece.trim().trim_start_matches("export ").trim_start_matches("let ");
        let Some(colon) = l.find(':') else { continue };
        let name = l[..colon].trim().trim_end_matches('?');
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') || name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        let values: Vec<String> = l[colon + 1..]
            .split('|')
            .filter_map(|v| {
                let v = v.trim().trim_end_matches([';', ',', '}']).trim();
                let q = v.chars().next()?;
                (matches!(q, '"' | '\'') && v.len() >= 2 && v.ends_with(q)).then(|| v[1..v.len() - 1].to_string())
            })
            .collect();
        if values.len() >= 2 {
            out.entry(name.to_string()).or_insert(values);
        }
    }
    let mut cva: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if let Some(at) = src.find("variants:") {
        let body = &src[at + 9..];
        let bytes = body.as_bytes();
        let (mut depth, mut i) = (0i32, 0usize);
        let mut current: Option<String> = None;
        let mut key = |k: &str, depth: i32, current: &mut Option<String>| {
            if depth == 1 {
                *current = Some(k.to_string());
            } else if depth == 2
                && let Some(c) = current.as_ref()
            {
                cva.entry(c.clone()).or_default().push(k.to_string());
            }
        };
        while i < bytes.len() {
            let c = bytes[i];
            match c {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth <= 0 {
                        break;
                    }
                    if depth == 1 {
                        current = None;
                    }
                }
                b'"' | b'\'' | b'`' => {
                    // A string: a quoted key when a colon follows, otherwise skipped (class lists
                    // like "hover:bg-primary" are values, not keys).
                    let close = body[i + 1..].find(c as char).map_or(bytes.len() - 1, |e| i + 1 + e);
                    if body[close + 1..].trim_start().starts_with(':') {
                        key(&body[i + 1..close], depth, &mut current);
                    }
                    i = close + 1;
                    continue;
                }
                c if c.is_ascii_alphabetic() || c == b'_' => {
                    let name = ident_at(&body[i..]).unwrap_or("");
                    if body[i + name.len()..].trim_start().starts_with(':') {
                        key(name, depth, &mut current);
                    }
                    i += name.len().max(1);
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
    }
    for (k, v) in cva {
        out.entry(k).or_insert(v);
    }
    out
}

/// The import path for a file: `@/…` when the project maps `@/*` to `src/*`, else relative.
fn import_path(rel: &str, alias: bool) -> String {
    let no_ext = rel.rsplit_once('.').map_or(rel, |(a, _)| a);
    let no_index = no_ext.strip_suffix("/index").unwrap_or(no_ext);
    match no_index.strip_prefix("src/") {
        Some(rest) if alias => format!("@/{rest}"),
        _ => format!("./{no_index}"),
    }
}

pub fn scan(root: &Path) -> Vec<CodeComponent> {
    let alias = ["tsconfig.json", "jsconfig.json", "tsconfig.app.json"]
        .iter()
        .filter_map(|f| std::fs::read_to_string(root.join(f)).ok())
        .any(|s| s.contains("\"@/*\""));
    let mut out = Vec::new();
    for path in files(root) {
        let Ok(src) = std::fs::read_to_string(&path) else { continue };
        let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
        let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("");
        let from = import_path(&rel, alias);
        let props = props(&src);
        if ext == "vue" || ext == "svelte" {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let name = pascal(stem);
            out.push(CodeComponent { import: format!("import {name} from \"{from}.{ext}\""), name, file: rel, props });
            continue;
        }
        for (name, default) in exports(&src) {
            let import = if default { format!("import {name} from \"{from}\"") } else { format!("import {{ {name} }} from \"{from}\"") };
            out.push(CodeComponent { name, file: rel.clone(), import, props: props.clone() });
        }
    }
    out
}

fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Suggestions for the design's components (`get_components` output) from the scanned code.
pub fn suggest(design: &Value, code: &[CodeComponent]) -> Vec<Value> {
    let mut targets: Vec<(String, String, BTreeMap<String, Vec<String>>)> = Vec::new();
    let mut seen_sets = HashSet::new();
    for s in design["componentSets"].as_array().into_iter().flatten() {
        let props: BTreeMap<String, Vec<String>> = s["properties"]
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect())).collect())
            .unwrap_or_default();
        targets.push((s["id"].as_str().unwrap_or("").to_string(), s["name"].as_str().unwrap_or("").to_string(), props));
        seen_sets.insert(s["name"].as_str().unwrap_or("").to_string());
    }
    for c in design["components"].as_array().into_iter().flatten() {
        if c.get("componentSet").is_some_and(|s| s.as_str().is_some_and(|s| seen_sets.contains(s))) {
            continue;
        }
        targets.push((c["id"].as_str().unwrap_or("").to_string(), c["name"].as_str().unwrap_or("").to_string(), BTreeMap::new()));
    }
    let mut out = Vec::new();
    for (id, name, design_props) in targets {
        // "Button/Primary" → try "ButtonPrimary", then the first and the last segment.
        let segments: Vec<&str> = name.split('/').map(str::trim).filter(|s| !s.is_empty()).collect();
        let mut keys = vec![norm(&name)];
        keys.extend(segments.first().map(|s| norm(s)));
        keys.extend(segments.last().map(|s| norm(s)));
        let mut ranked: Vec<(f32, &CodeComponent)> = code
            .iter()
            .filter_map(|cc| {
                let k = norm(&cc.name);
                let score = keys
                    .iter()
                    .enumerate()
                    .map(|(i, key)| {
                        let base = if *key == k {
                            1.0
                        } else if !k.is_empty() && key.len() > 2 && (key.contains(&k) || k.contains(key.as_str())) {
                            0.6
                        } else {
                            0.0
                        };
                        base - i as f32 * 0.05
                    })
                    .fold(0.0, f32::max);
                // Shallow, "components/ui" files are likelier to be the design system.
                let bonus = if cc.file.contains("components/") { 0.05 } else { 0.0 };
                (score > 0.0).then_some((score + bonus, cc))
            })
            .collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
        let Some((score, best)) = ranked.first().copied() else { continue };
        // Variant property → prop with the same values.
        let mut props = serde_json::Map::new();
        for (dp, values) in &design_props {
            let Some((cp, code_values)) = best.props.iter().max_by_key(|(_, cv)| values.iter().filter(|v| cv.iter().any(|c| norm(c) == norm(v))).count()) else { continue };
            let mut map = serde_json::Map::new();
            for v in values {
                if let Some(c) = code_values.iter().find(|c| norm(c) == norm(v)) {
                    map.insert(v.clone(), json!(format!("{cp}=\"{c}\"")));
                }
            }
            if !map.is_empty() {
                props.insert(dp.clone(), Value::Object(map));
            }
        }
        let mut call = json!({ "nodeId": id, "component": best.name, "import": best.import, "source": best.file });
        if !props.is_empty() {
            call["props"] = Value::Object(props);
        }
        out.push(json!({
            "design": { "id": id, "name": name },
            "confidence": (score.min(1.0) * 100.0).round() / 100.0,
            "add_code_connect_map": call,
            "alternatives": ranked.iter().skip(1).take(2).map(|(_, c)| json!({ "component": c.name, "source": c.file })).collect::<Vec<_>>(),
        }));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_exports_and_variant_props() {
        let src = r#"
import { cva } from "class-variance-authority";
const buttonVariants = cva("base", {
  variants: {
    variant: { default: "bg-primary hover:bg-primary/90", destructive: "b", ghost: "c" },
    size: { sm: "x", lg: "y" },
  },
});
export interface ButtonProps { variant?: "default" | "ghost"; disabled?: boolean }
// hover:bg-primary/90 in a class string must not read as a key.
export function Button(props: ButtonProps) { return null }
export { buttonVariants }
"#;
        assert_eq!(exports(src), vec![("Button".to_string(), false)]);
        let p = props(src);
        assert_eq!(p["variant"], vec!["default", "ghost"]);
        assert_eq!(p["size"], vec!["sm", "lg"]);
        let code = vec![CodeComponent { name: "Button".into(), file: "src/components/ui/button.tsx".into(), import: "import { Button } from \"@/components/ui/button\"".into(), props: p }];
        let design = json!({
            "componentSets": [{ "id": "4", "name": "Button", "properties": { "Variant": ["Default", "Ghost", "Link"] } }],
            "components": [{ "id": "5", "name": "Variant=Default", "componentSet": "Button" }, { "id": "9", "name": "Icons/Search" }],
        });
        let s = suggest(&design, &code);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0]["add_code_connect_map"]["props"]["Variant"]["Ghost"], "variant=\"ghost\"");
        assert_eq!(import_path("src/components/ui/button.tsx", true), "@/components/ui/button");
    }
}
