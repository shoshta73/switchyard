use std::{io::Stdout, sync::mpsc, time::Duration};

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use switchyard_core::{ChatRequest, Message, MessageRole, Model, Provider, Session};
use switchyard_crypto::argon2;
use tokio::runtime::Runtime;

use crate::{command, log, provider, terminal};

pub(crate) struct Data {
    pub(crate) logging_buffer: log::Buffer,
    pub(crate) encryption_key: argon2::Key,
    pub(crate) terminal: Option<Terminal<CrosstermBackend<Stdout>>>,
    pub(crate) terminal_guard: terminal::Guard,
    provider_events_tx: mpsc::Sender<ProviderEvent>,
    provider_events_rx: mpsc::Receiver<ProviderEvent>,
    runtime: Runtime,
}

enum ProviderEvent {
    InitialResponse {
        request_index: usize,
        status: u16,
        initial_response_time: Duration,
    },
    Chunk {
        message_index: usize,
        content: String,
    },
    ReasoningChunk {
        message_index: usize,
        content: String,
    },
    Complete {
        request_index: usize,
        total_time: Duration,
    },
    Failure {
        request_index: usize,
        message_index: usize,
        error: String,
        total_time: Duration,
    },
    ModelsDiscovered {
        provider: String,
        models: Vec<Model>,
    },
    ModelDiscoveryFailed {
        provider: String,
        error: String,
    },
}

impl Data {
    pub(crate) fn new() -> Self {
        let (provider_events_tx, provider_events_rx) = mpsc::channel();
        let data = Data {
            logging_buffer: log::setup(),
            encryption_key: Default::default(),
            terminal: None,
            terminal_guard: Default::default(),
            provider_events_tx,
            provider_events_rx,
            runtime: Runtime::new().expect("failed to create tokio runtime"),
        };
        log::startup_info();
        data
    }
}

pub(crate) struct State {
    devtools_visible: bool,
    devtools_tab: DevtoolsTab,
    input: String,
    session: Session,
    messages_scroll: u16,
    messages_follow_tail: bool,
    devtools_scroll: u16,
    requests: Vec<RequestLog>,
    model: Model,
    provider: Provider,
    menu: Option<Menu>,
}

struct Menu {
    kind: MenuKind,
    title: &'static str,
    items: Vec<String>,
    selected: usize,
}

#[derive(Clone, Copy)]
enum MenuKind {
    Provider,
    Model,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DevtoolsTab {
    Logs,
    Requests,
}

impl DevtoolsTab {
    fn toggle(self) -> Self {
        match self {
            Self::Logs => Self::Requests,
            Self::Requests => Self::Logs,
        }
    }
}

struct RequestLog {
    method: &'static str,
    url: String,
    model: String,
    status: Option<u16>,
    initial_response_time: Option<Duration>,
    total_time: Option<Duration>,
    error: Option<String>,
}

impl RequestLog {
    fn line(&self) -> String {
        let outcome = match (self.status, self.error.as_deref()) {
            (Some(status), _) => status.to_string(),
            (None, Some(error)) => format!("ERROR {error}"),
            (None, None) => "UNKNOWN".to_string(),
        };

        format!(
            "{} {} {} {} initial: {} total: {}",
            self.method,
            self.url,
            outcome,
            self.model,
            format_duration(self.initial_response_time),
            format_duration(self.total_time),
        )
    }
}

fn format_duration(duration: Option<Duration>) -> String {
    duration
        .map(|duration| format!("{}ms", duration.as_millis()))
        .unwrap_or_else(|| "pending".to_string())
}

impl Default for State {
    fn default() -> Self {
        Self {
            devtools_visible: false,
            devtools_tab: DevtoolsTab::Logs,
            input: String::new(),
            session: Session::default(),
            messages_scroll: 0,
            messages_follow_tail: true,
            devtools_scroll: u16::MAX,
            requests: vec![],
            model: provider::LocalProvider::model_from_env(),
            provider: Provider::from(provider::LocalProvider::from_env().name()),
            menu: None,
        }
    }
}

impl Menu {
    fn provider(current_provider: &Provider) -> Self {
        let items = vec!["Ollama".to_string(), "llama.cpp".to_string()];
        let selected = items
            .iter()
            .position(|item| item.eq_ignore_ascii_case(current_provider.name.as_str()))
            .unwrap_or(0);

        Self {
            kind: MenuKind::Provider,
            title: "Choose Provider",
            items,
            selected,
        }
    }

    fn model(current_provider: &Provider, current_model: &Model, models: Vec<Model>) -> Self {
        let default_model =
            provider::LocalProvider::default_model_for(current_provider.name.as_str());
        let mut items = Vec::new();
        push_menu_item(&mut items, current_model.name.as_str());
        push_menu_item(&mut items, default_model.name.as_str());
        for model in models {
            push_menu_item(&mut items, model.name.as_str());
        }

        Self {
            kind: MenuKind::Model,
            title: "Choose Model",
            items,
            selected: 0,
        }
    }

    fn previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn next(&mut self) {
        self.selected = (self.selected + 1).min(self.items.len().saturating_sub(1));
    }
}

fn push_menu_item(items: &mut Vec<String>, item: &str) {
    if !item.is_empty() && !items.iter().any(|existing| existing == item) {
        items.push(item.to_string());
    }
}

fn wrapped_line_count(text: &str, width: u16) -> u16 {
    let width = usize::from(width.max(1));
    text.split('\n')
        .map(|line| (line.chars().count().saturating_sub(1) / width) + 1)
        .sum::<usize>()
        .try_into()
        .unwrap_or(u16::MAX)
}

fn message_border_style(role: &MessageRole) -> Style {
    match role {
        MessageRole::User => Style::default().fg(Color::Blue),
        MessageRole::Assistant => Style::default().fg(Color::White),
        MessageRole::System => Style::default().fg(Color::Yellow),
        MessageRole::Tool => Style::default().fg(Color::Magenta),
        MessageRole::Diagnostic => Style::default().fg(Color::Red),
        MessageRole::Reasoning => Style::default().fg(Color::Cyan),
    }
}

fn devtools_tab_style(tab: DevtoolsTab, selected: DevtoolsTab) -> Style {
    if tab == selected {
        Style::default().fg(Color::Black).bg(Color::White)
    } else {
        Style::default()
    }
}

pub(crate) fn run(data: &mut Data, state: &mut State) -> Result<()> {
    loop {
        draw(data, state)?;

        if !update(data, state)? {
            break;
        }
    }

    Ok(())
}

fn draw(data: &mut Data, state: &mut State) -> Result<()> {
    let logs = data.logging_buffer.lines();
    let logs_display = Text::from(
        logs.iter()
            .map(|line| Line::from(log::line_spans(line)))
            .collect::<Vec<_>>(),
    );
    let requests_display = Text::from(
        state
            .requests
            .iter()
            .map(|request| Line::from(request.line()))
            .collect::<Vec<_>>(),
    );
    let terminal = data
        .terminal
        .as_mut()
        .context("terminal is not initialized")?;

    terminal.draw(|frame| {
        let area = frame.area();
        let prompt_inner_width = area.width.saturating_sub(2).max(1);
        let prompt_height = wrapped_line_count(&state.input, prompt_inner_width)
            .saturating_add(2)
            .clamp(3, area.height.min(8));
        let devtools_height = if state.devtools_visible {
            area.height
                .saturating_sub(prompt_height)
                .saturating_sub(1)
                .saturating_div(3)
                .max(3)
        } else {
            0
        };
        let chunks = if state.devtools_visible {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(1),
                    Constraint::Length(1),
                    Constraint::Length(prompt_height),
                    Constraint::Length(devtools_height),
                ])
                .split(area)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(1),
                    Constraint::Length(1),
                    Constraint::Length(prompt_height),
                ])
                .split(area)
        };

        let messages_width = chunks[0].width.max(1);
        let messages_height = usize::from(chunks[0].height.max(1));

        let messages_display = Text::from(
            state
                .session
                .messages
                .iter()
                .filter(|message| {
                    !matches!(message.role, MessageRole::Reasoning) || !message.content.is_empty()
                })
                .flat_map(|message| {
                    [
                        Line::from(message.content.as_str()),
                        Line::from(Span::styled(
                            "─".repeat(usize::from(messages_width)),
                            message_border_style(&message.role),
                        )),
                    ]
                })
                .collect::<Vec<_>>(),
        );

        let body = Paragraph::new(messages_display).wrap(Wrap { trim: false });
        let max_messages_scroll = body
            .line_count(messages_width)
            .saturating_sub(messages_height)
            .try_into()
            .unwrap_or(u16::MAX);
        state.messages_scroll = state.messages_scroll.min(max_messages_scroll);
        let body = body.scroll((state.messages_scroll, 0));

        let devtools_tabs = Paragraph::new(Line::from(vec![
            Span::styled(
                " Logs ",
                devtools_tab_style(DevtoolsTab::Logs, state.devtools_tab),
            ),
            Span::styled(
                " Requests ",
                devtools_tab_style(DevtoolsTab::Requests, state.devtools_tab),
            ),
        ]))
        .block(Block::default().title("Devtools").borders(Borders::ALL));

        let devtools_display = match state.devtools_tab {
            DevtoolsTab::Logs => logs_display.clone(),
            DevtoolsTab::Requests => requests_display.clone(),
        };

        let prompt = Paragraph::new(state.input.as_str())
            .block(Block::default().title("Prompt").borders(Borders::ALL))
            .wrap(Wrap { trim: false })
            .green();

        let status = Paragraph::new(Line::from(vec![
            Span::raw(format!("Model: {} ", state.model.name)),
            Span::styled(state.provider.name.as_str(), Style::default().dim()),
            Span::raw(format!(
                " | F12: {} devtools",
                if state.devtools_visible {
                    "hide"
                } else {
                    "show"
                }
            )),
            Span::raw(if state.devtools_visible {
                " | Tab: switch tab"
            } else {
                ""
            }),
        ]));

        frame.render_widget(body, chunks[0]);
        if state.devtools_visible {
            frame.render_widget(status, chunks[1]);
            frame.render_widget(prompt, chunks[2]);
            let devtools_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1)])
                .split(chunks[3]);
            let devtools = Paragraph::new(devtools_display).wrap(Wrap { trim: false });
            let max_devtools_scroll = devtools
                .line_count(devtools_chunks[1].width.max(1))
                .saturating_sub(usize::from(devtools_chunks[1].height.max(1)))
                .try_into()
                .unwrap_or(u16::MAX);
            state.devtools_scroll = state.devtools_scroll.min(max_devtools_scroll);
            let devtools = devtools.scroll((state.devtools_scroll, 0));
            frame.render_widget(devtools_tabs, devtools_chunks[0]);
            frame.render_widget(devtools, devtools_chunks[1]);
        } else {
            frame.render_widget(status, chunks[1]);
            frame.render_widget(prompt, chunks[2]);
        }

        if let Some(menu) = &state.menu {
            let area = centered_rect(area, 42, menu.items.len().saturating_add(4) as u16);
            let lines = menu
                .items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    if index == menu.selected {
                        Line::from(Span::styled(
                            format!("> {item}"),
                            Style::default().fg(Color::Black).bg(Color::White),
                        ))
                    } else {
                        Line::from(format!("  {item}"))
                    }
                })
                .collect::<Vec<_>>();
            let menu = Paragraph::new(Text::from(lines))
                .block(Block::default().title(menu.title).borders(Borders::ALL));
            frame.render_widget(Clear, area);
            frame.render_widget(menu, area);
        }
    })?;

    Ok(())
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

fn update(data: &mut Data, state: &mut State) -> Result<bool> {
    drain_provider_events(data, state);

    if !event::poll(Duration::from_millis(100))? {
        return Ok(true);
    }

    let Event::Key(key) = event::read()? else {
        return Ok(true);
    };

    if key.kind != KeyEventKind::Press {
        return Ok(true);
    }

    if state.menu.is_some() {
        return handle_menu_key(state, key.code);
    }

    match key.code {
        KeyCode::Esc => return Ok(false),

        KeyCode::F(12) => {
            state.devtools_visible = !state.devtools_visible;
            state.devtools_scroll = u16::MAX;
        }

        KeyCode::Tab => {
            if state.devtools_visible {
                state.devtools_tab = state.devtools_tab.toggle();
                state.devtools_scroll = u16::MAX;
            }
        }

        KeyCode::Up => {
            if state.devtools_visible {
                state.devtools_scroll = state.devtools_scroll.saturating_sub(1);
            } else {
                state.messages_scroll = state.messages_scroll.saturating_sub(1);
                state.messages_follow_tail = false;
            }
        }

        KeyCode::Down => {
            if state.devtools_visible {
                state.devtools_scroll = state.devtools_scroll.saturating_add(1);
            } else {
                state.messages_scroll = state.messages_scroll.saturating_add(1);
                state.messages_follow_tail = true;
            }
        }

        KeyCode::PageUp => {
            if state.devtools_visible {
                state.devtools_scroll = state.devtools_scroll.saturating_sub(5);
            } else {
                state.messages_scroll = state.messages_scroll.saturating_sub(5);
                state.messages_follow_tail = false;
            }
        }

        KeyCode::PageDown => {
            if state.devtools_visible {
                state.devtools_scroll = state.devtools_scroll.saturating_add(5);
            } else {
                state.messages_scroll = state.messages_scroll.saturating_add(5);
                state.messages_follow_tail = true;
            }
        }

        KeyCode::Enter => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                state.input.push('\n');
                return Ok(true);
            }

            let prompt = state.input.trim().to_string();

            let mut command_context = command::Context::new(&mut state.provider, &mut state.model);
            match command::handle(&mut command_context, prompt.as_str()) {
                command::Outcome::Exit => return Ok(false),
                command::Outcome::Handled => {
                    for diagnostic in command_context.take_diagnostics() {
                        push_diagnostic(state, diagnostic);
                    }
                    state.input.clear();
                    return Ok(true);
                }
                command::Outcome::OpenProviderMenu => {
                    state.menu = Some(Menu::provider(&state.provider));
                    state.input.clear();
                    return Ok(true);
                }
                command::Outcome::OpenModelMenu => {
                    open_model_menu(data, state);
                    state.input.clear();
                    return Ok(true);
                }
                command::Outcome::Ignored => {}
            }

            if !prompt.is_empty() {
                state.session.messages.push(Message {
                    role: MessageRole::User,
                    content: prompt.to_string(),
                });
                let request_messages = state.session.messages.clone();
                let reasoning_message_index = state.session.messages.len();
                state.session.messages.push(Message {
                    role: MessageRole::Reasoning,
                    content: String::new(),
                });
                let message_index = state.session.messages.len();
                state.session.messages.push(Message {
                    role: MessageRole::Assistant,
                    content: String::new(),
                });
                let local_provider =
                    provider::LocalProvider::from_name(state.provider.name.as_str());
                let request_url = local_provider.chat_url();
                let request_model = state.model.name.clone();
                let request_index = state.requests.len();
                state.requests.push(RequestLog {
                    method: "POST",
                    url: request_url,
                    model: request_model,
                    status: None,
                    initial_response_time: None,
                    total_time: None,
                    error: None,
                });
                let provider_events_tx = data.provider_events_tx.clone();
                let request = ChatRequest {
                    model: state.model.clone(),
                    messages: request_messages,
                };
                data.runtime.spawn(async move {
                    let started = std::time::Instant::now();
                    let result = local_provider
                        .stream_async(request, |event| match event {
                            provider::StreamEvent::InitialResponse {
                                status,
                                initial_response_time,
                            } => {
                                let _ = provider_events_tx.send(ProviderEvent::InitialResponse {
                                    request_index,
                                    status,
                                    initial_response_time,
                                });
                            }
                            provider::StreamEvent::Chunk(content) => {
                                let _ = provider_events_tx.send(ProviderEvent::Chunk {
                                    message_index,
                                    content,
                                });
                            }
                            provider::StreamEvent::ReasoningChunk(content) => {
                                let _ = provider_events_tx.send(ProviderEvent::ReasoningChunk {
                                    message_index: reasoning_message_index,
                                    content,
                                });
                            }
                            provider::StreamEvent::Complete { total_time } => {
                                let _ = provider_events_tx.send(ProviderEvent::Complete {
                                    request_index,
                                    total_time,
                                });
                            }
                        })
                        .await;

                    if let Err(err) = result {
                        let _ = provider_events_tx.send(ProviderEvent::Failure {
                            request_index,
                            message_index,
                            error: format!("{err:#}"),
                            total_time: started.elapsed(),
                        });
                    }
                });
                state.messages_scroll = u16::MAX;
                state.messages_follow_tail = true;
            }

            state.input.clear();
        }

        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'j' {
                state.input.push('\n');
            } else {
                state.input.push(c);
            }
        }

        KeyCode::Backspace => {
            state.input.pop();
        }

        _ => {}
    }

    Ok(true)
}

fn push_diagnostic(state: &mut State, content: String) {
    state.session.messages.push(Message {
        role: MessageRole::Diagnostic,
        content,
    });
    state.messages_scroll = u16::MAX;
    state.messages_follow_tail = true;
}

fn open_model_menu(data: &mut Data, state: &mut State) {
    state.menu = Some(Menu::model(&state.provider, &state.model, Vec::new()));

    let provider_name = state.provider.name.clone();
    let local_provider = provider::LocalProvider::from_name(provider_name.as_str());
    let provider_events_tx = data.provider_events_tx.clone();
    data.runtime.spawn(async move {
        match local_provider.models_async().await {
            Ok(models) => {
                let _ = provider_events_tx.send(ProviderEvent::ModelsDiscovered {
                    provider: provider_name,
                    models,
                });
            }
            Err(err) => {
                let _ = provider_events_tx.send(ProviderEvent::ModelDiscoveryFailed {
                    provider: provider_name,
                    error: format!("{err:#}"),
                });
            }
        }
    });
}

fn handle_menu_key(state: &mut State, code: KeyCode) -> Result<bool> {
    match code {
        KeyCode::Esc => {
            state.menu = None;
        }
        KeyCode::Up => {
            if let Some(menu) = &mut state.menu {
                menu.previous();
            }
        }
        KeyCode::Down => {
            if let Some(menu) = &mut state.menu {
                menu.next();
            }
        }
        KeyCode::Enter => {
            if let Some(menu) = state.menu.take() {
                let value = menu.items[menu.selected].clone();
                match menu.kind {
                    MenuKind::Provider => {
                        let local_provider = provider::LocalProvider::from_name(value.as_str());
                        state.provider = Provider::from(local_provider.name());
                        state.model =
                            provider::LocalProvider::default_model_for(local_provider.name());
                        push_diagnostic(
                            state,
                            format!(
                                "Provider set to {}. Model set to {}.",
                                state.provider.name, state.model.name
                            ),
                        );
                    }
                    MenuKind::Model => {
                        state.model = value.into();
                        push_diagnostic(state, format!("Model set to {}.", state.model.name));
                    }
                }
            }
        }
        _ => {}
    }

    Ok(true)
}

fn drain_provider_events(data: &mut Data, state: &mut State) {
    while let Ok(event) = data.provider_events_rx.try_recv() {
        match event {
            ProviderEvent::InitialResponse {
                request_index,
                status,
                initial_response_time,
            } => {
                if let Some(request) = state.requests.get_mut(request_index) {
                    request.status = Some(status);
                    request.initial_response_time = Some(initial_response_time);
                }
            }
            ProviderEvent::Chunk {
                message_index,
                content,
            } => {
                if let Some(message) = state.session.messages.get_mut(message_index) {
                    message.content.push_str(content.as_str());
                }
            }
            ProviderEvent::ReasoningChunk {
                message_index,
                content,
            } => {
                if let Some(message) = state.session.messages.get_mut(message_index) {
                    message.content.push_str(content.as_str());
                }
            }
            ProviderEvent::Complete {
                request_index,
                total_time,
            } => {
                if let Some(request) = state.requests.get_mut(request_index) {
                    request.total_time = Some(total_time);
                    request.error = None;
                }
            }
            ProviderEvent::Failure {
                request_index,
                message_index,
                error,
                total_time,
            } => {
                if let Some(request) = state.requests.get_mut(request_index) {
                    request.status = None;
                    request.total_time = Some(total_time);
                    request.error = Some(error.clone());
                }
                if let Some(message) = state.session.messages.get_mut(message_index)
                    && message.content.is_empty()
                {
                    message.content = "[request failed]".to_string();
                }
                state.session.messages.push(Message {
                    role: MessageRole::Diagnostic,
                    content: format!("{} request failed: {error}", state.provider.name),
                });
            }
            ProviderEvent::ModelsDiscovered { provider, models } => {
                if provider == state.provider.name
                    && let Some(menu) = &mut state.menu
                    && matches!(menu.kind, MenuKind::Model)
                {
                    *menu = Menu::model(&state.provider, &state.model, models);
                }
            }
            ProviderEvent::ModelDiscoveryFailed { provider, error } => {
                if provider == state.provider.name {
                    push_diagnostic(state, format!("Model discovery failed: {error}"));
                }
            }
        }
        if state.messages_follow_tail {
            state.messages_scroll = u16::MAX;
        }
        state.devtools_scroll = u16::MAX;
    }
}

#[cfg(test)]
mod tests {
    use switchyard_core::{Model, Provider};

    use super::Menu;

    #[test]
    fn model_menu_includes_current_default_and_discovered_models_once() {
        let provider = Provider::from("Ollama");
        let current_model = Model::from("custom");

        let menu = Menu::model(
            &provider,
            &current_model,
            vec!["llama3.2".into(), "qwen2.5".into(), "custom".into()],
        );

        assert_eq!(menu.items, vec!["custom", "llama3.2", "qwen2.5"]);
    }
}
