//! In-place TOML field patching using `toml_edit`.
//!
//! Reads the file, modifies only the requested key, and writes it back —
//! preserving all other keys, comments, and whitespace.

use std::path::Path;

/// Patch a single field in a TOML config file without touching any other keys.
///
/// * `path`    – path to the `.toml` file (created if absent)
/// * `section` – top-level table name, e.g. `"emby"` or `"dev"`
/// * `key`     – key within that table
/// * `value`   – new value as a `serde_json::Value`; `Null` removes the key
pub fn patch_field(
    path: &Path,
    section: &str,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    let content = if path.exists() {
        std::fs::read_to_string(path)
            .map_err(|e| format!("read config: {e}"))?
    } else {
        String::new()
    };

    let mut doc = content
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| format!("parse toml: {e}"))?;

    // Ensure the section table exists.
    if !doc.contains_key(section) {
        let mut tbl = toml_edit::Table::new();
        tbl.set_implicit(true);
        doc.insert(section, toml_edit::Item::Table(tbl));
    }

    let tbl = doc[section]
        .as_table_mut()
        .ok_or_else(|| format!("[{section}] is not a table"))?;

    match json_to_toml_item(value) {
        Ok(Some(item)) => {
            tbl.insert(key, item);
        }
        Ok(None) => {
            tbl.remove(key);
        }
        Err(e) => return Err(e),
    }

    // Create parent dirs if needed.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create config dir: {e}"))?;
    }

    std::fs::write(path, doc.to_string())
        .map_err(|e| format!("write config: {e}"))
}

fn json_to_toml_item(
    v: &serde_json::Value,
) -> Result<Option<toml_edit::Item>, String> {
    match v {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Bool(b) => Ok(Some(toml_edit::value(*b))),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Some(toml_edit::value(i)))
            } else if let Some(f) = n.as_f64() {
                Ok(Some(toml_edit::value(f)))
            } else {
                Err(format!("unrepresentable number: {n}"))
            }
        }
        serde_json::Value::String(s) => Ok(Some(toml_edit::value(s.as_str()))),
        serde_json::Value::Array(arr) => {
            let mut a = toml_edit::Array::new();
            for item in arr {
                match item {
                    serde_json::Value::String(s) => a.push(s.as_str()),
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            a.push(i);
                        } else if let Some(f) = n.as_f64() {
                            a.push(f);
                        }
                    }
                    serde_json::Value::Bool(b) => a.push(*b),
                    _ => {
                        return Err(
                            "nested objects in arrays not supported".to_owned()
                        );
                    }
                }
            }
            Ok(Some(toml_edit::value(a)))
        }
        serde_json::Value::Object(_) => {
            Err("nested TOML objects not supported via patch_field".to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_creates_and_preserves() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");

        // Create initial content.
        std::fs::write(&path, "[emby]\nplayer = \"mpv\"\n").expect("write");

        // Patch a bool field.
        patch_field(
            &path,
            "emby",
            "fullscreen",
            &serde_json::Value::Bool(true),
        )
        .expect("patch bool");
        let content = std::fs::read_to_string(&path).expect("read");
        assert!(content.contains("fullscreen = true"), "bool patch missing");
        assert!(content.contains("player = \"mpv\""), "existing key removed");

        // Patch an array field.
        patch_field(
            &path,
            "dev",
            "version_prefer",
            &serde_json::json!(["VCB", "ANi"]),
        )
        .expect("patch array");
        let content = std::fs::read_to_string(&path).expect("read");
        assert!(content.contains("version_prefer"), "array patch missing");

        // Remove a field with null.
        patch_field(&path, "emby", "fullscreen", &serde_json::Value::Null)
            .expect("patch null");
        let content = std::fs::read_to_string(&path).expect("read");
        assert!(!content.contains("fullscreen"), "null should remove key");
    }

    #[test]
    fn patch_creates_file_when_absent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("new.toml");
        patch_field(
            &path,
            "emby",
            "player",
            &serde_json::Value::String("iina".to_owned()),
        )
        .expect("patch on new file");
        let content = std::fs::read_to_string(&path).expect("read");
        assert!(
            content.contains("player = \"iina\""),
            "patch on new file failed"
        );
    }
}
