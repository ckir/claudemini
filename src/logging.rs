use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use tracing_appender::rolling;
use std::fs;
use std::path::Path;
use sysinfo::System;
use std::panic;
use backtrace::Backtrace;
use fs2::FileExt;

pub fn init() -> (tracing_appender::non_blocking::WorkerGuard, fs::File) {
    let log_dir = "./flight_recorder/";
    if !Path::new(log_dir).exists() {
        fs::create_dir_all(log_dir).expect("failed to create log directory");
    }

    // Singleton Lock
    let lock_file_path = Path::new(log_dir).join("claudemini.lock");
    let lock_file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&lock_file_path)
        .expect("Failed to open lock file");

    if let Err(_) = lock_file.try_lock_exclusive() {
        eprintln!("❌ Error: Another instance of Claudemini is already running.");
        eprintln!("Please quit all other Claudemini, claude-cli, or gemini-cli processes and run claudemini again.");
        std::process::exit(1);
    }

    let pid = std::process::id();
    let file_appender = rolling::daily(log_dir, format!("claudemini.log.{}", pid));
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace")))
        .with(fmt::layer().json().with_writer(non_blocking))
        .init();

    setup_panic_hook();
    log_system_snapshot();
    
    tracing::info!(pid, "Flight recorder initialized at {}", log_dir);
    
    (guard, lock_file)
}

fn setup_panic_hook() {
    panic::set_hook(Box::new(|panic_info| {
        let backtrace = Backtrace::new();
        let location = panic_info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_else(|| "unknown".to_string());
        let payload = panic_info.payload();
        let message = if let Some(s) = payload.downcast_ref::<&str>() {
            *s
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.as_str()
        } else {
            "Box<Any>"
        };

        tracing::error!(
            panic_message = message,
            panic_location = %location,
            backtrace = ?backtrace,
            "APPLICATION PANIC DETECTED"
        );
    }));
}

fn log_system_snapshot() {
    let mut sys = System::new_all();
    sys.refresh_all();

    tracing::info!(
        os_name = ?System::name(),
        os_version = ?System::os_version(),
        kernel_version = ?System::kernel_version(),
        host_name = ?System::host_name(),
        cpu_count = sys.cpus().len(),
        total_memory_kb = sys.total_memory(),
        used_memory_kb = sys.used_memory(),
        "System Snapshot"
    );

    // Log environment variables (redacting potentially sensitive ones)
    for (key, value) in std::env::vars() {
        let is_sensitive = key.to_lowercase().contains("key") 
            || key.to_lowercase().contains("secret") 
            || key.to_lowercase().contains("token")
            || key.to_lowercase().contains("auth");
        
        if is_sensitive {
            tracing::debug!(env_var = %key, env_value = "[REDACTED]", "Environment Variable");
        } else {
            tracing::debug!(env_var = %key, env_value = %value, "Environment Variable");
        }
    }
}
