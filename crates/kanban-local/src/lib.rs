use std::path::PathBuf;

pub fn default_db_path() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kb")
        .join("kb.db")
}

pub fn default_log_dir() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kb")
        .join("logs")
}

pub fn default_actor() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "local".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_paths_match_kb_data_layout() {
        let db_path = default_db_path();
        assert!(db_path.ends_with("kb/kb.db"));

        let log_dir = default_log_dir();
        assert!(log_dir.ends_with("kb/logs"));
    }
}
