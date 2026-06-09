use std::sync::atomic::{AtomicBool, Ordering};

use tracing::{debug, info};

use crate::runtime;

static DIRECT_TERMINAL_OUTPUT_ENABLED: AtomicBool = AtomicBool::new(true);

mod detail {
    #[derive(Default)]
    pub struct LogBufferInner {
        pub lines: std::collections::VecDeque<String>,
        pub current: String,
    }

    pub struct LogBufferWriter {
        pub buffer: crate::log::Buffer,
    }

    const MAX_LOG_LINES: usize = 1_000;

    impl std::io::Write for LogBufferWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let text = String::from_utf8_lossy(buf);
            let mut inner = self.buffer.inner.lock().expect("log buffer poisoned");

            for c in text.chars() {
                if c == '\n' {
                    let line = std::mem::take(&mut inner.current);
                    inner.lines.push_back(line);

                    if inner.lines.len() > MAX_LOG_LINES {
                        inner.lines.pop_front();
                    }
                } else {
                    inner.current.push(c);
                }
            }

            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    pub fn level_color(level: &str) -> ratatui::style::Color {
        use ratatui::style::Color::{Blue, Green, Magenta, Red, White, Yellow};
        match level.trim() {
            "ERROR" => Red,
            "WARN" => Yellow,
            "INFO" => Green,
            "DEBUG" => Blue,
            "TRACE" => Magenta,
            _ => White,
        }
    }

    pub fn split_once_inclusive_whitespace(text: &str) -> Option<(&str, &str)> {
        let split_at = text.find(char::is_whitespace)? + 1;
        Some(text.split_at(split_at))
    }

    pub fn split_leading_whitespace(text: &str) -> Option<(&str, &str)> {
        let split_at = text.find(|c: char| !c.is_whitespace())?;
        Some(text.split_at(split_at))
    }

    pub enum DirectTerminalWriter {
        Stderr(std::io::Stderr),
        Sink(std::io::Sink),
    }

    impl std::io::Write for DirectTerminalWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            match self {
                DirectTerminalWriter::Stderr(stderr) => stderr.write(buf),
                DirectTerminalWriter::Sink(sink) => sink.write(buf),
            }
        }

        fn flush(&mut self) -> std::io::Result<()> {
            match self {
                DirectTerminalWriter::Stderr(stderr) => stderr.flush(),
                DirectTerminalWriter::Sink(sink) => sink.flush(),
            }
        }
    }

    pub struct DirectTerminalMakeWriter;

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for DirectTerminalMakeWriter {
        type Writer = DirectTerminalWriter;

        fn make_writer(&'a self) -> Self::Writer {
            if crate::log::DIRECT_TERMINAL_OUTPUT_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
            {
                DirectTerminalWriter::Stderr(std::io::stderr())
            } else {
                DirectTerminalWriter::Sink(std::io::sink())
            }
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct Buffer {
    inner: std::sync::Arc<std::sync::Mutex<detail::LogBufferInner>>,
}

impl Buffer {
    pub fn lines(&self) -> Vec<String> {
        let inner = self.inner.lock().expect("log buffer poisoned");
        let mut lines = inner.lines.iter().cloned().collect::<Vec<_>>();

        if !inner.current.is_empty() {
            lines.push(inner.current.clone());
        }

        lines
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Buffer {
    type Writer = detail::LogBufferWriter;

    fn make_writer(&'a self) -> Self::Writer {
        detail::LogBufferWriter {
            buffer: self.clone(),
        }
    }
}

pub(crate) fn line_spans(line: &str) -> Vec<ratatui::text::Span<'static>> {
    use ratatui::{
        style::{
            Color::{DarkGray, White},
            Style,
        },
        text::Span,
    };

    use detail::{level_color, split_leading_whitespace, split_once_inclusive_whitespace};

    let Some((timestamp, after_timestamp)) = split_once_inclusive_whitespace(line) else {
        return vec![Span::styled(line.to_string(), Style::default().fg(White))];
    };

    let Some((level_padding, level_and_message)) = split_leading_whitespace(after_timestamp) else {
        return vec![
            Span::styled(timestamp.to_string(), Style::default().fg(DarkGray)),
            Span::styled(after_timestamp.to_string(), Style::default().fg(White)),
        ];
    };

    let Some((level, message)) = split_once_inclusive_whitespace(level_and_message) else {
        return vec![
            Span::styled(timestamp.to_string(), Style::default().fg(DarkGray)),
            Span::styled(after_timestamp.to_string(), Style::default().fg(White)),
        ];
    };

    vec![
        Span::styled(timestamp.to_string(), Style::default().fg(DarkGray)),
        Span::raw(level_padding.to_string()),
        Span::styled(level.to_string(), Style::default().fg(level_color(level))),
        Span::styled(message.to_string(), Style::default().fg(White)),
    ]
}

pub(crate) fn setup() -> Buffer {
    use tracing_appender::rolling;
    use tracing_subscriber::{
        EnvFilter, Layer, fmt::layer, layer::SubscriberExt, registry, util::SubscriberInitExt,
    };

    let log_buffer = Buffer::default();
    let file_appender = rolling::never("/tmp", "switchyard.log");
    let env_filter = if cfg!(debug_assertions) {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"))
    } else {
        EnvFilter::new("info")
    };

    let file_layer = layer()
        .json()
        .with_ansi(false)
        .with_writer(file_appender)
        .with_filter(env_filter.clone());

    let tui_layer = layer()
        .with_ansi(false)
        .with_writer(log_buffer.clone())
        .with_filter(env_filter);

    let direct_terminal_layer = layer()
        .with_ansi(true)
        .with_writer(detail::DirectTerminalMakeWriter)
        .with_filter(if cfg!(debug_assertions) {
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"))
        } else {
            EnvFilter::new("info")
        });

    registry()
        .with(file_layer)
        .with(tui_layer)
        .with(direct_terminal_layer)
        .init();

    log_buffer
}

pub(crate) fn disable_direct_terminal_output() {
    DIRECT_TERMINAL_OUTPUT_ENABLED.store(false, Ordering::Relaxed);
}

pub(crate) fn startup_info() {
    info!("Starting SwitchYard {}", env!("SWITCHYARD_VERSION"));
    debug!("Core version: {}", switchyard_core::meta::VERSION);
    debug!("Crypto version: {}", switchyard_crypto::meta::VERSION);
    runtime::log_debug_info();
}
