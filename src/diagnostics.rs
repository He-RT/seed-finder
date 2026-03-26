use std::{
    fs,
    io::Write,
    panic,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn install_diagnostics() {
    panic::set_hook(Box::new(|info| {
        log_diag(&format!("panic: {info}"));
    }));
}

pub fn log_diag(message: &str) {
    static LOG_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let lock = LOG_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().ok();

    let path = diagnostic_log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let line = format!("[{}] {message}\n", timestamp_string());
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}

fn diagnostic_log_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join("Documents")
            .join("seed-finder.log");
    }
    PathBuf::from("/tmp/seed-finder.log")
}

fn timestamp_string() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => format!("{}.{}", duration.as_secs(), duration.subsec_millis()),
        Err(_) => "0.000".into(),
    }
}
