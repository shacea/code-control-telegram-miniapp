use std::fs;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

/// Return ~/Projects
pub fn projects_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join("Projects"))
}

/// List immediate subdirectories of ~/Projects
pub fn list_project_folders(limit: usize) -> Vec<String> {
    let Some(projects) = projects_dir() else {
        return vec![];
    };

    let _ = fs::create_dir_all(&projects);

    let mut dirs: Vec<String> = vec![];
    if let Ok(entries) = fs::read_dir(&projects) {
        for entry in entries.flatten() {
            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') {
                        continue;
                    }
                    dirs.push(name);
                    if dirs.len() >= limit {
                        break;
                    }
                }
            }
        }
    }

    dirs.sort();
    dirs
}

pub fn encode_folders_b64(folders: &[String]) -> String {
    let json = serde_json::to_string(folders).unwrap_or_else(|_| "[]".to_string());
    URL_SAFE_NO_PAD.encode(json.as_bytes())
}

/// Validate path is within ~/Projects and is a directory
pub fn validate_projects_subdir(path: &str) -> Result<String, String> {
    let Some(projects) = projects_dir() else {
        return Err("cannot determine home directory".to_string());
    };
    let _ = fs::create_dir_all(&projects);

    let p = Path::new(path);
    let canon = p.canonicalize().map_err(|e| format!("invalid path: {e}"))?;

    let projects_canon = projects
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize ~/Projects: {e}"))?;

    if !canon.starts_with(&projects_canon) {
        return Err("path must be within ~/Projects".to_string());
    }

    if !canon.is_dir() {
        return Err("path is not a directory".to_string());
    }

    Ok(canon.display().to_string())
}

/// Turn a folder name (one component) into absolute path under ~/Projects
pub fn folder_name_to_path(folder: &str) -> Result<String, String> {
    if folder.is_empty() {
        return Err("folder is empty".to_string());
    }
    if folder.contains('/') || folder.contains('\\') || folder.contains("..") {
        return Err("invalid folder".to_string());
    }
    let Some(projects) = projects_dir() else {
        return Err("cannot determine home directory".to_string());
    };
    let abs = projects.join(folder);
    validate_projects_subdir(&abs.display().to_string())
}
