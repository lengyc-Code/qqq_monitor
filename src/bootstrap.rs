use qqq_engine::data_directory;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub fn logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("qqq_monitor=info,qqq_engine=info,qqq_platform_windows=info,wgpu_core=warn,wgpu_hal=error"));
    let logs = data_directory().join("logs");
    if let Err(error) = std::fs::create_dir_all(&logs) {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .init();
        tracing::warn!(%error, "无法创建日志目录，使用标准错误输出");
        return None;
    }
    match tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("qqq-monitor")
        .max_log_files(7)
        .build(logs)
    {
        Ok(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_ansi(false)
                        .with_writer(writer),
                )
                .init();
            Some(guard)
        }
        Err(_) => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_writer(std::io::stderr)
                .init();
            None
        }
    }
}
