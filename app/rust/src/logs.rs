use std::{
    fs::{self, File},
    io,
    path::Path,
    sync::{mpsc, LazyLock, Mutex},
    thread,
};

use crate::frb_generated::StreamSink;
use anyhow::Result;
use file_rotate::{
    compression::Compression,
    suffix::{AppendTimestamp, FileLimit},
    {ContentLimit, FileRotate},
};
use log::Log;
use simplelog::{ConfigBuilder, LevelFilter, WriteLogger};

/// 保存 Dart 端注册的 StreamSink（Option），线程安全
pub static FLUTTER_LOGGER: LazyLock<Mutex<Option<StreamSink<String>>>> =
    LazyLock::new(|| Mutex::new(None));

/// 日志消息 channel 的 Sender（放在 LazyLock<Mutex<Option<_>>> 中以便安全初始化和访问）
pub static LOG_SENDER: LazyLock<Mutex<Option<mpsc::Sender<String>>>> =
    LazyLock::new(|| Mutex::new(None));

pub struct MainLogger {
    write_logger: Box<WriteLogger<FileRotate<AppendTimestamp>>>,
}

impl MainLogger {
    fn new(write_logger: Box<WriteLogger<FileRotate<AppendTimestamp>>>) -> Self {
        Self { write_logger }
    }
}

impl Log for MainLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        // 先把日志写到本地 rolling 文件（同步）
        self.write_logger.log(record);

        // 再把要发回 Dart 的字符串放到 channel（非阻塞尽量）
        let message = format!(
            "{}:{} -- {}",
            record.level(),
            record.target(),
            record.args()
        );

        if let Some(tx) = LOG_SENDER.lock().unwrap().as_ref() {
            // best-effort：如果发送失败（channel closed），直接忽略
            let _ = tx.send(message);
        } else {
            // 如果还没有 dispatcher，降级输出（避免丢失）
            eprintln!("{}", message);
        }
    }

    fn flush(&self) {}
}

pub fn init(cache_dir: &str) -> Result<()> {
    let path = Path::new(cache_dir).join("logs/main.log");
    let log = FileRotate::new(
        path,
        AppendTimestamp::default(FileLimit::MaxFiles(3)),
        ContentLimit::Lines(1000),
        Compression::None,
        #[cfg(unix)]
        None,
    );
    let config = ConfigBuilder::new().set_time_format_rfc3339().build();
    let write_logger = WriteLogger::new(LevelFilter::Info, config, log);
    let main_logger = MainLogger::new(write_logger);
    log::set_boxed_logger(Box::new(main_logger))?;
    log::set_max_level(LevelFilter::Info);

    // 启动 dispatcher（若尚未启动）
    init_dispatcher();

    Ok(())
}

/// 确保 dispatcher 存在：创建 channel 并 spawn 一个线程专门把消息发送回 Dart
fn init_dispatcher() {
    let mut guard = LOG_SENDER.lock().unwrap();
    if guard.is_some() {
        return;
    }

    let (tx, rx) = mpsc::channel::<String>();
    *guard = Some(tx);

    // 克隆 FLUTTER_LOGGER 的引用
    let flutter_logger_ref = &*FLUTTER_LOGGER;

    thread::spawn(move || {
        // dispatcher loop，阻塞在 recv 上
        while let Ok(msg) = rx.recv() {
            // 最先写到 stderr 方便 native 层调试
            eprintln!("[DISPATCH] {}", msg);

            // 读取当前注册的 sink（clone 出来以避免长时间持锁）
            let maybe_sink = {
                let guard = flutter_logger_ref.lock().unwrap();
                guard.clone()
            };

            if let Some(mut sink) = maybe_sink {
                // best-effort：如果 sink.add 失败，忽略错误，继续处理下一条
                // 注意：sink.add 可能会阻塞，若 Dart 端非常慢，dispatcher 线程会被阻塞，但不会造成 Dart <-> native 的同步死锁回路
                let _ = sink.add(msg);
            }
        }
        // 若 rx 被关闭（所有 Sender dropped），线程退出
    });
}

/// 当 Dart 注册 StreamSink 时调用
pub fn set_flutter_sink(sink: StreamSink<String>) {
    let mut guard = FLUTTER_LOGGER.lock().unwrap();
    *guard = Some(sink);
}

pub fn export(cache_dir: &str, target_file_path: &str) -> Result<()> {
    let mut zip = zip::ZipWriter::new(File::create(target_file_path)?);
    let default_options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    let log_folder = Path::new(cache_dir).join("logs/");
    for entry in (fs::read_dir(&log_folder)?).flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.strip_prefix(cache_dir)?.to_str() {
                zip.start_file(name, default_options)?;
                let mut log_file = File::open(path)?;
                io::copy(&mut log_file, &mut zip)?;
            }
        }
    }

    zip.finish()?;
    Ok(())
}
