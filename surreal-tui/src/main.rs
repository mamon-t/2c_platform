use anyhow::Result;
use crossterm::{
    event::{Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Terminal,
};
// ВАЖНО: Импортируем и Client (для типа), и Ws (для схемы подключения)
use surrealdb::engine::remote::ws::Client;
use surrealdb::Surreal;
use std::io;

// ============================================================================
// Конфигурация подключения
// ============================================================================

#[derive(Clone)]
struct Config {
    host: String,
    user: String,
    pass: String,
    ns: String,
    db: String,
}

impl Config {
    fn from_env() -> Self {
        // Агрессивная очистка от любых мусорных символов, пробелов и кавычек
        let clean = |v: Result<String, std::env::VarError>, default: &str| {
            v.unwrap_or_else(|_| default.to_string())
             .trim()
             .trim_matches(|c: char| c.is_control() || c.is_whitespace() || c == '"' || c == '\'')
             .to_string()
        };

        Self {
            host: clean(std::env::var("SURREAL_HOST"), "192.168.31.31:8000"),
            user: clean(std::env::var("SURREAL_USER"), "root"),
            pass: clean(std::env::var("SURREAL_PASS"), "root"),
            ns: clean(std::env::var("SURREAL_NS"), "main"),
            db: clean(std::env::var("SURREAL_DB"), "main"),
        }
    }

    // Переименовали в endpoint_url, чтобы подчеркнуть: SDK ждет строго "host:port"
    fn endpoint_url(&self) -> String {
        let h = self.host.trim();
        // Если пользователь всё-таки вписал ws://, мы это принудительно убираем,
        // так как тип Ws сам добавит схему и путь /rpc.
        h.strip_prefix("ws://")
         .or_else(|| h.strip_prefix("wss://"))
         .unwrap_or(h)
         .to_string()
    }
}

// ============================================================================
// Состояние приложения
// ============================================================================

struct App {
    config: Config,
    // ВАЖНО: Здесь должен быть Client, а не Ws
    db: Option<Surreal<Client>>,
    
    query: String,
    cursor_position: usize,
    result: String,
    status: String,
    
    settings_open: bool,
    settings_fields: Vec<String>,
    settings_active: usize,
    
    completion_open: bool,
    completion_items: Vec<String>,
    completion_selected: usize,
    
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        let config = Config::from_env();
        Self {
            config: config.clone(),
            db: None,
            query: String::from("SELECT * FROM company;"),
            cursor_position: 0,
            result: String::from("Нажмите F5 для выполнения запроса..."),
            status: "Отключено. F2 - настройки подключения".to_string(),
            settings_open: false,
            settings_fields: vec![
                config.host.clone(), config.user.clone(), config.pass.clone(),
                config.ns.clone(), config.db.clone(),
            ],
            settings_active: 0,
            completion_open: false,
            completion_items: Vec::new(),
            completion_selected: 0,
            should_quit: false,
        }
    }

    async fn connect(&mut self) {
        let endpoint = self.config.endpoint_url();

        self.result = format!("Попытка подключения.\nEndpoint: '{}'\n", endpoint);
        self.status = "Подключение...".to_string();

        // Передаем чистый "host:port" в тип Ws, как показано в оф. примере SDK
        let db: Surreal<Client> = match Surreal::new::<surrealdb::engine::remote::ws::Ws>(endpoint.as_str()).await {
            Ok(db) => db,
            Err(e) => {
                self.result += format!("Ошибка подключения к:{}\nДетали:\n{:?}\n", endpoint, e).as_str();
                self.status = "Ошибка подключения".to_string();
                self.db = None;
                return;
            }
        };


        if let Err(e) = db.signin(surrealdb::opt::auth::Root {
            username: self.config.user.trim().to_string(),
            password: self.config.pass.trim().to_string(),
        }).await {
            self.result += format!("Ошибка авторизации:\n{}", e).as_str();
            self.status = "Ошибка авторизации".to_string();
            self.db = None;
            return;
        }

        if let Err(e) = db.use_ns(self.config.ns.trim()).use_db(self.config.db.trim()).await {
            self.result += format!("Ошибка выбора БД:\n{}\n", e).as_str();
            self.status = "Ошибка".to_string();
            self.db = None;
            return;
        }

        self.db = Some(db);
        self.status += format!("Подключено: {} ({}/{})",
                              self.config.host, self.config.ns, self.config.db).as_str();
        self.result = "Подключение успешно. Нажмите F5 для выполнения запроса.".to_string();
    }

    async fn execute_query(&mut self) {
        if self.db.is_none() {
            self.connect().await;
            if self.db.is_none() {
                return;
            }
        }

        self.status = "Выполнение запроса...".to_string();
        
        let mut query = self.query.trim().to_string();
        if !query.ends_with(';') {
            query.push(';');
        }

        let db = self.db.as_ref().unwrap();
        match db.query(query).await {
            Ok(mut response) => {
                // ВАЖНО: явное указание типа Option<serde_json::Value> для take
                let result: serde_json::Value = match response.take::<Option<serde_json::Value>>(0) {
                    Ok(Some(val)) => val,
                    Ok(None) => serde_json::json!([]),
                    Err(e) => serde_json::json!({"error": e.to_string()}),
                };
                
                self.result = serde_json::to_string_pretty(&result)
                    .unwrap_or_else(|_| "Ошибка форматирования JSON".to_string());
                self.status = "Успешно выполнено".to_string();
            }
            Err(e) => {
                self.result = format!("Ошибка выполнения запроса:\n{}", e);
                self.status = "Ошибка".to_string();
            }
        }
    }

    fn open_settings(&mut self) {
        self.settings_fields = vec![
            self.config.host.clone(), self.config.user.clone(), self.config.pass.clone(),
            self.config.ns.clone(), self.config.db.clone(),
        ];
        self.settings_active = 0;
        self.settings_open = true;
    }

    fn save_settings(&mut self) {
        self.config.host = self.settings_fields[0].clone();
        self.config.user = self.settings_fields[1].clone();
        self.config.pass = self.settings_fields[2].clone();
        self.config.ns = self.settings_fields[3].clone();
        self.config.db = self.settings_fields[4].clone();
        self.settings_open = false;
        self.db = None;
    }

    fn get_current_word(&self) -> String {
        if self.query.is_empty() || self.cursor_position == 0 {
            return String::new();
        }

        // Позиция курсора минус 1 (чтобы попасть на символ под курсором)
        let pos = self.cursor_position - 1;

        // Находим начало слова
        let word_start = self.query[..char_to_byte_idx(&self.query, pos)]
            .rfind(|c: char| !c.is_alphanumeric() && c != '_')
            .map(|i| i + 1)
            .unwrap_or(0);

        // Считаем символы от начала строки до word_start
        let char_start = self.query[..word_start].chars().count();

        // Находим конец слова
        let byte_pos = char_to_byte_idx(&self.query, pos);
        let word_end = self.query[byte_pos..]
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .map(|i| byte_pos + i)
            .unwrap_or(self.query.len());

        let char_end = self.query[..word_end].chars().count();

        self.query[char_to_byte_idx(&self.query, char_start)..char_to_byte_idx(&self.query, char_end)].to_string()
    }

    fn update_completions(&mut self) {
        let word = self.get_current_word().to_uppercase();
        if word.is_empty() {
            self.completion_open = false;
            return;
        }

        self.completion_items = KEYWORDS.iter()
            .filter(|kw| kw.starts_with(&word) && **kw != word)
            .take(10)
            .map(|kw| kw.to_string())
            .collect();

        self.completion_open = !self.completion_items.is_empty();
        self.completion_selected = 0;
    }

    fn apply_completion(&mut self) {
        if !self.completion_open || self.completion_items.is_empty() {
            return;
        }

        let selected = &self.completion_items[self.completion_selected];
        let word = self.get_current_word();

        // Находим байтовую позицию начала слова
        let word_char_start = self.cursor_position - word.chars().count();
        let byte_start = char_to_byte_idx(&self.query, word_char_start);
        let byte_end = char_to_byte_idx(&self.query, self.cursor_position);

        self.query.replace_range(byte_start..byte_end, selected);
        self.cursor_position = word_char_start + selected.chars().count();
        self.completion_open = false;
    }
}

// ============================================================================
// Подсветка синтаксиса
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
enum TokenKind {
    Keyword, String, Number, Ident, Punct, Whitespace,
}

const KEYWORDS: &[&str] = &[
    "SELECT", "FROM", "WHERE", "CREATE", "UPDATE", "DELETE", "INSERT", "INTO",
    "SET", "LET", "IF", "THEN", "ELSE", "END", "DEFINE", "RETURN", "TABLE",
    "FIELD", "INDEX", "ON", "RELATE", "REMOVE", "INFO", "SHOW", "USE", "ORDER",
    "BY", "GROUP", "LIMIT", "START", "FETCH", "SPLIT", "TIMEOUT", "PARALLEL",
    "AFTER", "BEFORE", "DIFF", "VALUE", "VALUES", "ONLY", "WHEN", "AS", "DISTINCT",
    "OMIT", "EXPLAIN", "OVERWRITE", "UNIQUE", "PERMISSIONS", "CHANGEFEED", "TYPE",
    "DEFAULT", "READONLY", "DROP", "SCHEMAFULL", "SCHEMALESS", "BEGIN", "TRANSACTION",
    "COMMIT", "CANCEL", "THROW", "BREAK", "CONTINUE", "FOR", "IN", "CONTAINS",
    "CONTAINSALL", "CONTAINSANY", "CONTAINSNONE", "CONTAINSNOT", "INSIDE", "NOTINSIDE",
    "INTERSECTS", "ALL", "ANY", "NONE", "SOME", "AND", "OR", "NOT", "XOR", "IS",
    "NULL", "TRUE", "FALSE", "RECORD", "GEOMETRY", "UUID", "DURATION", "FUTURE",
];

fn tokenize(input: &str) -> Vec<(TokenKind, String)> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();
    
    while let Some(&(i, c)) = chars.peek() {
        if c.is_whitespace() {
            let start = i;
            let mut end = i;
            while let Some(&(j, c)) = chars.peek() {
                if !c.is_whitespace() { break; }
                end = j + c.len_utf8();
                chars.next();
            }
            tokens.push((TokenKind::Whitespace, input[start..end].to_string()));
        } else if c == '\'' || c == '"' {
            let quote = c;
            let start = i;
            chars.next();
            let mut end = i + c.len_utf8();
            while let Some(&(j, c)) = chars.peek() {
                end = j + c.len_utf8();
                chars.next();
                if c == quote { break; }
            }
            tokens.push((TokenKind::String, input[start..end].to_string()));
        } else if c.is_ascii_digit() {
            let start = i;
            let mut end = i;
            while let Some(&(j, c)) = chars.peek() {
                if !c.is_ascii_digit() && c != '.' { break; }
                end = j + c.len_utf8();
                chars.next();
            }
            tokens.push((TokenKind::Number, input[start..end].to_string()));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            let mut end = i;
            while let Some(&(j, c)) = chars.peek() {
                if !c.is_alphanumeric() && c != '_' { break; }
                end = j + c.len_utf8();
                chars.next();
            }
            let word = input[start..end].to_string();
            let kind = if KEYWORDS.contains(&word.to_uppercase().as_str()) {
                TokenKind::Keyword
            } else {
                TokenKind::Ident
            };
            tokens.push((kind, word));
        } else {
            tokens.push((TokenKind::Punct, c.to_string()));
            chars.next();
        }
    }
    tokens
}

// ============================================================================
// Справка
// ============================================================================

fn get_help_url(keyword: &str) -> Option<&'static str> {
    match keyword.to_uppercase().as_str() {
        "SELECT" => Some("https://surrealdb.com/docs/surrealql/statements/select"),
        "CREATE" => Some("https://surrealdb.com/docs/surrealql/statements/create"),
        "UPDATE" => Some("https://surrealdb.com/docs/surrealql/statements/update"),
        "DELETE" => Some("https://surrealdb.com/docs/surrealql/statements/delete"),
        "INSERT" => Some("https://surrealdb.com/docs/surrealql/statements/insert"),
        "RELATE" => Some("https://surrealdb.com/docs/surrealql/statements/relate"),
        "DEFINE" => Some("https://surrealdb.com/docs/surrealql/statements/define"),
        "REMOVE" => Some("https://surrealdb.com/docs/surrealql/statements/remove"),
        "INFO" => Some("https://surrealdb.com/docs/surrealql/statements/info"),
        "IF" => Some("https://surrealdb.com/docs/surrealql/statements/if"),
        "FOR" => Some("https://surrealdb.com/docs/surrealql/statements/for"),
        "LET" => Some("https://surrealdb.com/docs/surrealql/statements/let"),
        "RETURN" => Some("https://surrealdb.com/docs/surrealql/statements/return"),
        "BEGIN" => Some("https://surrealdb.com/docs/surrealql/statements/begin"),
        "COMMIT" => Some("https://surrealdb.com/docs/surrealql/statements/commit"),
        "CANCEL" => Some("https://surrealdb.com/docs/surrealql/statements/cancel"),
        "WHERE" => Some("https://surrealdb.com/docs/surrealql/clauses/where"),
        "ORDER" => Some("https://surrealdb.com/docs/surrealql/clauses/order"),
        "GROUP" => Some("https://surrealdb.com/docs/surrealql/clauses/group"),
        "LIMIT" => Some("https://surrealdb.com/docs/surrealql/clauses/limit"),
        "FETCH" => Some("https://surrealdb.com/docs/surrealql/clauses/fetch"),
        _ => None,
    }
}

/// Вычисляет визуальную позицию курсора (x, y) с учётом переносов
fn calculate_cursor_position(text: &str, char_idx: usize, line_width: u16) -> (usize, usize) {
    let mut x = 0;
    let mut y = 0;
    let lw = line_width as usize;

    for (i, c) in text.chars().enumerate() {
        if i >= char_idx {
            break;
        }
        if c == '\n' {
            x = 0;
            y += 1;
        } else {
            x += 1;
            if lw > 0 && x >= lw {
                x = 0;
                y += 1;
            }
        }
    }

    (x, y)
}

// ============================================================================
// Интерфейс
// ============================================================================

fn ui(f: &mut ratatui::Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),   // Статус (1 строка)
            Constraint::Min(10),     // Две панели (command + output)
            Constraint::Length(1),   // Нижняя строка с хоткеями
        ])
        .split(f.size());

    // Статус (1 строка)
    let status_style = if app.status.contains("Ошибка") {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    };
    let status = Paragraph::new(app.status.clone())
        .style(status_style);
    f.render_widget(status, chunks[0]);

    // Две панели ГОРИЗОНТАЛЬНО (как в NC!)
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),  // Левая панель (command)
            Constraint::Percentage(50),  // Правая панель (output)
        ])
        .split(chunks[1]);

    // Левая панель: ввод запроса
    // ВАЖНО: разбиваем текст по \n и создаём Vec<Line>
    let lines: Vec<Line> = app.query.split('\n').map(|line| {
        let tokens = tokenize(line);
        let spans: Vec<Span> = tokens.into_iter().map(|(kind, text)| {
            let style = match kind {
                TokenKind::Keyword => Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                TokenKind::String => Style::default().fg(Color::Green),
                TokenKind::Number => Style::default().fg(Color::Yellow),
                TokenKind::Ident => Style::default().fg(Color::Cyan),
                TokenKind::Punct => Style::default().fg(Color::White),
                TokenKind::Whitespace => Style::default(),
            };
            Span::styled(text, style)
        }).collect();
        Line::from(spans)
    }).collect();

    let input = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Command (F5-выполнить, F2-настройки, F1-справка, Tab-автодоп) "));
    f.render_widget(input, panels[0]);

    // Позиционирование курсора: считаем реальную строку и колонку
    let byte_idx = char_to_byte_idx(&app.query, app.cursor_position);
    let text_before = &app.query[..byte_idx];
    let current_line = text_before.lines().count().saturating_sub(1);
    let line_start = text_before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col_in_line = app.query[line_start..byte_idx].chars().count();

    f.set_cursor(
        panels[0].x + 1 + col_in_line as u16,
        panels[0].y + 1 + current_line as u16,
    );

    // Правая панель: результат
    let result = Paragraph::new(app.result.clone())
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL).title(" Output "))
        .wrap(Wrap { trim: false });
    f.render_widget(result, panels[1]);

    // Нижняя строка с хоткеями (как в NC!)
    let help_text = Line::from(vec![
        Span::styled("F1", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Help  "),
        Span::styled("F2", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Settings  "),
        Span::styled("F5", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Execute  "),
        Span::styled("F10", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Quit"),
    ]);
    let help = Paragraph::new(help_text)
        .style(Style::default().bg(Color::DarkGray));
    f.render_widget(help, chunks[2]);

    // Автодополнение (поверх левой панели)
    if app.completion_open && !app.completion_items.is_empty() {
        let popup_area = Rect {
            x: panels[0].x + 2,
            y: panels[0].y + 2,
            width: 30.min(panels[0].width.saturating_sub(4)),
            height: (app.completion_items.len() as u16 + 2).min(panels[0].height.saturating_sub(4)),
        };
        f.render_widget(Clear, popup_area);

        let items: Vec<Line> = app.completion_items.iter().enumerate().map(|(i, item)| {
            let style = if i == app.completion_selected {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default()
            };
            Line::from(Span::styled(item.clone(), style))
        }).collect();

        let popup = Paragraph::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Автодополнение "));
        f.render_widget(popup, popup_area);
    }

    // Модалка настроек (поверх всего)
    if app.settings_open {
        let popup_area = Rect {
            x: f.size().width / 6,
            y: f.size().height / 4,
            width: f.size().width * 2 / 3,
            height: f.size().height / 2,
        };
        f.render_widget(Clear, popup_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Настройки (Tab-переключение, Enter-сохранить, Esc-отмена) ");
        f.render_widget(block.clone(), popup_area);

        let inner = block.inner(popup_area);
        let inner_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), Constraint::Length(3), Constraint::Length(3),
                Constraint::Length(3), Constraint::Length(3), Constraint::Min(0),
            ])
            .split(inner);

        for (i, label) in ["Host", "User", "Password", "Namespace", "Database"].iter().enumerate() {
            let style = if i == app.settings_active {
                Style::default().fg(Color::Yellow).bg(Color::DarkGray)
            } else {
                Style::default()
            };
            let input = Paragraph::new(app.settings_fields[i].clone())
                .style(style)
                .block(Block::default().borders(Borders::ALL).title(*label));
            f.render_widget(input, inner_chunks[i]);
        }
    }
}

// ============================================================================
// Обработчики ввода
// ============================================================================

fn handle_completion_input(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Tab => app.completion_open = false,
        KeyCode::Up => {
            if app.completion_selected > 0 { app.completion_selected -= 1; }
        }
        KeyCode::Down => {
            if app.completion_selected < app.completion_items.len().saturating_sub(1) {
                app.completion_selected += 1;
            }
        }
        KeyCode::Enter => { app.apply_completion(); }
        _ => {}
    }
}

/// Вычисляет визуальную позицию курсора (x, y) с учётом переносов строк
/// Конвертирует индекс символа в байтовый индекс для безопасной работы с UTF-8
fn char_to_byte_idx(s: &str, char_idx: usize) -> usize {
    s.char_indices()
     .nth(char_idx)
     .map(|(i, _)| i)
     .unwrap_or(s.len())
}

async fn handle_main_input(app: &mut App, key: KeyEvent) {
    if app.settings_open {
        match key.code {
            KeyCode::Esc => app.settings_open = false,
            KeyCode::Tab | KeyCode::Down => app.settings_active = (app.settings_active + 1) % 5,
            KeyCode::BackTab | KeyCode::Up => {
                app.settings_active = if app.settings_active == 0 { 4 } else { app.settings_active - 1 };
            }
            KeyCode::Enter => {
                app.save_settings();
                app.connect().await;
            }
            KeyCode::Backspace => { app.settings_fields[app.settings_active].pop(); }
            KeyCode::Char(c) => { app.settings_fields[app.settings_active].push(c); }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::F(10) => app.should_quit = true,
        KeyCode::F(1) => {
            let word = app.get_current_word();
            if let Some(url) = get_help_url(&word) {
                let _ = open::that(url);
                app.status = format!("Открыта справка: {}", url);
            } else {
                app.status = "Справка не найдена для этого слова".to_string();
            }
        }
        KeyCode::F(2) => app.open_settings(),
        KeyCode::F(5) => app.execute_query().await,
        KeyCode::Tab => { app.update_completions(); }

        KeyCode::Enter => {
            let byte_idx = char_to_byte_idx(&app.query, app.cursor_position);
            let text_before = &app.query[..byte_idx];
            let current_line = text_before.lines().last().unwrap_or("");

            if !current_line.trim_end().ends_with(';') && !current_line.trim_end().ends_with('\\') {
                app.query.insert(byte_idx, '\\');
                app.query.insert(byte_idx + 1, '\n');
                app.cursor_position += 2;
            } else {
                app.query.insert(byte_idx, '\n');
                app.cursor_position += 1;
            }
        }

        KeyCode::Backspace => {
            if app.cursor_position > 0 {
                app.cursor_position -= 1;
                let byte_idx = char_to_byte_idx(&app.query, app.cursor_position);
                let next_byte_idx = char_to_byte_idx(&app.query, app.cursor_position + 1);
                app.query.replace_range(byte_idx..next_byte_idx, "");
            }
        }

        KeyCode::Delete => {
            let total_chars = app.query.chars().count();
            if app.cursor_position < total_chars {
                let byte_idx = char_to_byte_idx(&app.query, app.cursor_position);
                let next_byte_idx = char_to_byte_idx(&app.query, app.cursor_position + 1);
                app.query.replace_range(byte_idx..next_byte_idx, "");
            }
        }

        KeyCode::Left => {
            if app.cursor_position > 0 {
                app.cursor_position -= 1;
            }
        }

        KeyCode::Right => {
            let total_chars = app.query.chars().count();
            if app.cursor_position < total_chars {
                app.cursor_position += 1;
            }
        }

        KeyCode::Up => {
            let byte_idx = char_to_byte_idx(&app.query, app.cursor_position);
            let text_before = &app.query[..byte_idx];
            let current_line_start = text_before.rfind('\n').map(|i| i + 1).unwrap_or(0);
            let col_in_current_line = byte_idx - current_line_start;

            if current_line_start == 0 {
                return;
            }

            let prev_line_start = text_before[..current_line_start.saturating_sub(1)]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);

            let prev_line_end = current_line_start.saturating_sub(1);
            let prev_line_len = prev_line_end - prev_line_start;
            let target_col = col_in_current_line.min(prev_line_len);
            let new_byte_idx = prev_line_start + target_col;

            app.cursor_position = app.query[..new_byte_idx].chars().count();
        }

        KeyCode::Down => {
            let byte_idx = char_to_byte_idx(&app.query, app.cursor_position);
            let text_before = &app.query[..byte_idx];
            let current_line_start = text_before.rfind('\n').map(|i| i + 1).unwrap_or(0);
            let col_in_current_line = byte_idx - current_line_start;

            let current_line_end = app.query[byte_idx..]
                .find('\n')
                .map(|i| byte_idx + i)
                .unwrap_or(app.query.len());

            if current_line_end >= app.query.len() {
                return;
            }

            let next_line_start = current_line_end + 1;
            let next_line_end = app.query[next_line_start..]
                .find('\n')
                .map(|i| next_line_start + i)
                .unwrap_or(app.query.len());

            let next_line_len = next_line_end - next_line_start;
            let target_col = col_in_current_line.min(next_line_len);
            let new_byte_idx = next_line_start + target_col;

            app.cursor_position = app.query[..new_byte_idx].chars().count();
        }

        KeyCode::Char(c) => {
            if key.modifiers == KeyModifiers::CONTROL && c == 'c' {
                app.should_quit = true;
            } else {
                let byte_idx = char_to_byte_idx(&app.query, app.cursor_position);
                app.query.insert(byte_idx, c);
                app.cursor_position += 1;
            }
        }
        _ => {}
    }
}

// ============================================================================
// Основное (main)
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let mut events = crossterm::event::EventStream::new();

    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Some(Ok(Event::Key(key))) = events.next().await {
            if app.settings_open {
                match key.code {
                    KeyCode::Esc => app.settings_open = false,
                    KeyCode::Tab | KeyCode::Down => app.settings_active = (app.settings_active + 1) % 5,
                    KeyCode::BackTab | KeyCode::Up => {
                        app.settings_active = if app.settings_active == 0 { 4 } else { app.settings_active - 1 };
                    }
                    KeyCode::Enter => {
                        app.save_settings();
                        app.connect().await;
                    }
                    KeyCode::Backspace => { app.settings_fields[app.settings_active].pop(); }
                    KeyCode::Char(c) => { app.settings_fields[app.settings_active].push(c); }
                    _ => {}
                }
            } else if app.completion_open {
                handle_completion_input(&mut app, key);
            } else {
                handle_main_input(&mut app, key).await;
            }
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    
    Ok(())
}