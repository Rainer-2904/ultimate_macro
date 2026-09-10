use std::path::PathBuf;
#[cfg(target_os = "android")]
static DATA_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
pub fn init(path: PathBuf) {
    let _ = DATA_DIR.set(path);
}

// Android's working directory is not writable. Keep both SQLite and logs in the
// app's private files directory; desktop retains its existing relative paths.
pub fn path(name: &str) -> PathBuf {
    #[cfg(target_os = "android")]
    return DATA_DIR
        .get()
        .expect("Android storage must be initialized before startup")
        .join(name);
    #[cfg(not(target_os = "android"))]
    PathBuf::from(name)
}
