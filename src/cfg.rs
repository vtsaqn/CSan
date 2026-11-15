use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Cài đặt dạng key=value (thay cho SharedPreferences "cfg").
#[derive(Clone)]
pub struct Cfg {
    pub map: BTreeMap<String, String>,
    pub path: PathBuf,
}

impl Cfg {
    pub fn load(dir: &Path) -> Cfg {
        let path = dir.join("config.txt");
        let mut map = BTreeMap::new();
        if let Ok(t) = std::fs::read_to_string(&path) {
            for l in t.lines() {
                if let Some((k, v)) = l.split_once('=') {
                    map.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }
        Cfg { map, path }
    }
    pub fn str(&self, k: &str, d: &str) -> String {
        self.map.get(k).cloned().unwrap_or_else(|| d.to_string())
    }
    pub fn int(&self, k: &str, d: i64) -> i64 {
        self.map.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
    }
    pub fn boolean(&self, k: &str, d: bool) -> bool {
        match self.map.get(k).map(|s| s.as_str()) {
            Some("1") | Some("true") => true,
            Some("0") | Some("false") => false,
            _ => d,
        }
    }
    pub fn set(&mut self, k: &str, v: String) {
        self.map.insert(k.to_string(), v);
    }
    pub fn save(&self) {
        if let Some(p) = self.path.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let mut s = String::new();
        for (k, v) in &self.map {
            s.push_str(&format!("{}={}\n", k, v));
        }
        let _ = std::fs::write(&self.path, s);
    }
}
