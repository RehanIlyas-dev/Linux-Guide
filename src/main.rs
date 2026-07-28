use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::thread;

use clap::Parser;
use crossterm::ExecutableCommand;
use crossterm::cursor::SetCursorStyle;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const BANNER: &[&str] = &[
    "\u{2501}",
    "",
    "     LINUX COMMAND GUIDE     ",
    "    Interactive Command Explorer",
    "",
    "\u{2501}",
];

const BANNER_COLORS: &[Color] = &[
    Color::Rgb(147, 112, 219),
    Color::Black,
    Color::Yellow,
    Color::Rgb(147, 112, 219),
    Color::Black,
    Color::Rgb(147, 112, 219),
];

#[derive(Parser)]
#[command(name = "linux-guide", version = VERSION, about = "TUI Linux command explorer")]
struct Args {
    #[arg(help = "Command to look up (optional - launches TUI if omitted)")]
    command: Option<String>,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    if let Some(cmd) = args.command {
        one_shot(&cmd);
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    terminal.show_cursor()?;
    terminal.backend_mut().execute(SetCursorStyle::SteadyBar)?;

    let res = event_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    let _ = io::stdout().execute(LeaveAlternateScreen);
    terminal.show_cursor()?;

    if let Err(e) = res {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

fn one_shot(cmd: &str) {
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    if tokens.len() > 1 && tokens[1..].iter().any(|a| a.starts_with('-')) {
        let (lines, _, _) = build_command_output(cmd);
        for line in &lines {
            println!("{line}");
        }
        return;
    }

    let lookup = if tokens.len() > 1 {
        tokens.join("-")
    } else {
        cmd.to_string()
    };

    match describe_command(&lookup) {
        Some(desc) => println!("{}", desc),
        None => {
            if is_command_available(cmd) {
                eprintln!("'{}' exists but no description available.", cmd);
            } else {
                eprintln!("linux-guide: '{}' is not a valid command.", cmd);
                let s = find_suggestions(cmd);
                if !s.is_empty() {
                    eprintln!("\nDid you mean?");
                    for name in &s {
                        eprintln!("  - {}", name);
                    }
                }
            }
        }
    }
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    terminal.clear()?;

    loop {
        if let Some(ref rx) = app.pending
            && let Ok((lines, output_cmd, show_man)) = rx.try_recv()
        {
            app.output = lines;
            app.output_cmd = output_cmd;
            app.show_man_prompt = show_man;
            app.pending = None;
        }

        terminal.draw(|f| render(f, app))?;

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Enter => {
                    app.accept_suggestion();
                    let cmd = app.input.trim().to_string();
                    if !cmd.is_empty() {
                        app.input.clear();
                        app.suggestions.clear();
                        app.selected_suggestion = None;
                        app.scroll = 0;
                        app.show_man_prompt = false;
                        app.output_cmd.clear();
                        app.output = vec![
                            format!(" Searching cheat.sh for '{}'...", cmd),
                            String::new(),
                            "    Fetching...".into(),
                        ];

                        let (tx, rx) = mpsc::channel();
                        let cmd_clone = cmd.clone();
                        thread::spawn(move || {
                            let result = build_command_output(&cmd_clone);
                            let _ = tx.send(result);
                        });
                        app.pending = Some(rx);
                    }
                }
                KeyCode::Char('\t') => app.accept_suggestion(),
                KeyCode::Char('m') if app.show_man_prompt => {
                    let cmd = app.output_cmd.clone();
                    if !cmd.is_empty() {
                        suspend_tui(terminal)?;
                        open_man_page(&cmd);
                        suspend_return(terminal)?;
                        app.output = vec![format!("Closed man page for '{}'.", cmd)];
                        app.show_man_prompt = false;
                    }
                }
                KeyCode::Backspace => app.pop_char(),
                KeyCode::Char(c) => app.push_char(c),
                KeyCode::Up if !app.suggestions.is_empty() => app.prev_suggestion(),
                KeyCode::Down if !app.suggestions.is_empty() => app.next_suggestion(),
                KeyCode::Up => app.scroll = app.scroll.saturating_sub(1),
                KeyCode::Down => app.scroll = app.scroll.saturating_add(1),
                KeyCode::PageUp => app.scroll = app.scroll.saturating_sub(10),
                KeyCode::PageDown => app.scroll = app.scroll.saturating_add(10),
                KeyCode::Home => app.scroll = 0,
                _ => {}
            }
        }
    }

    Ok(())
}

fn suspend_tui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    disable_raw_mode()?;
    Ok(())
}

fn suspend_return(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    enable_raw_mode()?;
    terminal.backend_mut().execute(EnterAlternateScreen)?;
    terminal.hide_cursor()?;
    terminal.clear()?;
    Ok(())
}

struct App {
    input: String,
    output: Vec<String>,
    scroll: u16,
    show_man_prompt: bool,
    output_cmd: String,

    all_commands: Vec<String>,
    suggestions: Vec<String>,
    selected_suggestion: Option<usize>,

    pending: Option<mpsc::Receiver<(Vec<String>, String, bool)>>,
}

impl App {
    fn new() -> Self {
        let commands = load_command_list();

        Self {
            input: String::new(),
            output: vec![
                format!("  Welcome to Linux Guide v{}", VERSION),
                String::new(),
                "  Type a command and press Enter to learn about it".into(),
                "  Suggestions appear as you type".into(),
                "  Press M to view the man page for the last result".into(),
                "  Press Q to quit".into(),
            ],
            scroll: 0,
            show_man_prompt: false,
            output_cmd: String::new(),
            all_commands: commands,
            suggestions: Vec::new(),
            selected_suggestion: None,
            pending: None,
        }
    }

    fn push_char(&mut self, c: char) {
        self.input.push(c);
        self.refresh_suggestions();
    }

    fn pop_char(&mut self) {
        self.input.pop();
        self.refresh_suggestions();
    }

    fn refresh_suggestions(&mut self) {
        let input = self.input.trim().to_lowercase();
        if input.is_empty() {
            self.suggestions.clear();
            self.selected_suggestion = None;
            return;
        }

        let mut matches: Vec<&String> = self
            .all_commands
            .iter()
            .filter(|c| c.starts_with(&input))
            .collect();

        matches.sort_by(|a, b| a.len().cmp(&b.len()).then(a.cmp(b)));
        matches.truncate(10);

        self.suggestions = matches.into_iter().cloned().collect();
        self.selected_suggestion = if self.suggestions.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    fn accept_suggestion(&mut self) {
        if let Some(idx) = self.selected_suggestion
            && idx < self.suggestions.len()
        {
            self.input = self.suggestions[idx].clone();
            self.suggestions.clear();
            self.selected_suggestion = None;
        }
    }

    fn prev_suggestion(&mut self) {
        let idx = self.selected_suggestion.unwrap_or(0);
        self.selected_suggestion = Some(idx.saturating_sub(1));
    }

    fn next_suggestion(&mut self) {
        let max = self.suggestions.len().saturating_sub(1);
        let idx = self.selected_suggestion.unwrap_or(0);
        self.selected_suggestion = Some((idx + 1).min(max));
    }
}

fn build_command_output(cmd: &str) -> (Vec<String>, String, bool) {
    let mut lines = Vec::new();
    let mut output_cmd = String::new();
    let mut show_man = false;

    let tokens: Vec<&str> = cmd.split_whitespace().collect();

    if tokens.len() > 1 && tokens[1..].iter().any(|a| a.starts_with('-')) {
        let base = tokens[0];
        let args = &tokens[1..];

        match describe_command(base) {
            Some(desc) => {
                lines.push(format!("[{}]", base));
                let is_online = desc.lines().count() > 10;
                if is_online {
                    lines.push("   Source: cheat.sh".to_string());
                }
                lines.push(String::new());
                for line in desc.lines().take(3) {
                    lines.push(format!("   {}", line));
                }
                output_cmd = base.to_string();
                show_man = can_open_man();
            }
            None => {
                lines.push(format!(" '{}' (unknown command)", base));
            }
        }

        lines.push(String::new());
        lines.push("   Arguments:".into());
        let mut context = base.to_string();
        for arg in args {
            if arg.starts_with('-') {
                let flag = arg.split('=').next().unwrap_or(arg);
                let flag_url = format!("{}/{}", context, flag);
                let desc = fetch_online(&flag_url).ok();
                let note = match desc {
                    Some(t) if !t.is_empty() && !t.starts_with("Unknown topic") => {
                        t.lines().next().unwrap_or("").to_string()
                    }
                    _ => "no description".to_string(),
                };
                lines.push(format!("     {}  {}", arg, note));
            } else {
                context = format!("{}-{}", context, arg);
                let desc = fetch_online(&context).ok();
                let note = match desc {
                    Some(t) if !t.is_empty() && !t.starts_with("Unknown topic") => {
                        t.lines().next().unwrap_or("").to_string()
                    }
                    _ => String::new(),
                };
                if note.is_empty() {
                    lines.push(format!("     {}  (argument)", arg));
                } else {
                    lines.push(format!("     {}  {}", arg, note));
                }
            }
        }

        (lines, output_cmd, show_man)
    } else if tokens.len() > 1 {
        let hyphenated = tokens.join("-");
        match describe_command(&hyphenated) {
            Some(desc) => {
                lines.push(hyphenated.to_string());
                let is_online = desc.lines().count() > 10;
                if is_online {
                    lines.push("   Source: cheat.sh".to_string());
                }
                lines.push(String::new());
                for line in desc.lines() {
                    lines.push(format!("   {}", line));
                }
                output_cmd = hyphenated.to_string();
                show_man = can_open_man();
            }
            None => {
                lines.push(format!(" '{}' is not a valid command.", hyphenated));
                let s = find_suggestions(&hyphenated);
                if !s.is_empty() {
                    lines.push(String::new());
                    lines.push("   Did you mean?".into());
                    for name in &s {
                        lines.push(format!("     {} {}", "\u{2192}", name));
                    }
                }
            }
        }

        (lines, output_cmd, show_man)
    } else {
        match describe_command(cmd) {
            Some(desc) => {
                lines.push(cmd.to_string());
                let is_online = desc.lines().count() > 10;
                if is_online {
                    lines.push("   Source: cheat.sh".to_string());
                }
                lines.push(String::new());
                for line in desc.lines() {
                    lines.push(format!("   {}", line));
                }
                output_cmd = cmd.to_string();
                show_man = can_open_man();
            }
            None => {
                if is_command_available(cmd) {
                    lines.push(format!(" '{}' exists but no description.", cmd));
                    output_cmd = cmd.to_string();
                    show_man = can_open_man();
                } else {
                    lines.push(format!(" '{}' is not a valid command.", cmd));
                    let s = find_suggestions(cmd);
                    if !s.is_empty() {
                        lines.push(String::new());
                        lines.push("   Did you mean?".into());
                        for name in &s {
                            lines.push(format!("     {} {}", "\u{2192}", name));
                        }
                    }
                }
            }
        }

        (lines, output_cmd, show_man)
    }
}

fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

fn can_open_man() -> bool {
    !is_windows()
}

fn load_command_list() -> Vec<String> {
    if !is_windows()
        && let Ok(output) = run_cmd("bash", &["-c", "compgen -c"])
    {
        let mut cmds: Vec<String> = output.lines().map(|l| l.to_string()).collect();
        cmds.sort();
        cmds.dedup();
        if !cmds.is_empty() {
            return cmds;
        }
    }

    COMMON_COMMANDS.iter().map(|s| s.to_string()).collect()
}

fn describe_command(cmd: &str) -> Option<String> {
    if let Ok(text) = fetch_online(cmd)
        && !text.is_empty()
        && !text.starts_with("Unknown topic")
    {
        return Some(text);
    }

    if !is_windows()
        && let Ok(desc) = run_cmd("whatis", &[cmd])
        && !desc.is_empty()
    {
        return Some(desc);
    }

    if is_windows() {
        return describe_windows(cmd);
    }

    None
}

fn cache_dir() -> PathBuf {
    let base = if cfg!(target_os = "linux") {
        std::env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                PathBuf::from(home).join(".cache")
            })
    } else if cfg!(target_os = "macos") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join("Library/Caches")
    } else if cfg!(target_os = "windows") {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\Temp"))
    } else {
        PathBuf::from("/tmp")
    };
    base.join("linux-guide")
}

fn cache_key(cmd: &str) -> String {
    cmd.replace(|c: char| !c.is_alphanumeric(), "_")
}

fn cache_load(cmd: &str) -> Option<String> {
    let path = cache_dir().join(cache_key(cmd));
    fs::read_to_string(&path).ok()
}

fn cache_save(cmd: &str, text: &str) {
    let dir = cache_dir();
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join(cache_key(cmd));
    let _ = fs::write(&path, text);
}

fn fetch_online(cmd: &str) -> Result<String, String> {
    if let Some(cached) = cache_load(cmd) {
        return Ok(cached);
    }

    let url = format!("https://cheat.sh/{cmd}?T");

    let url = url.replace(' ', "%20");

    let output = Command::new("curl")
        .args(["-s", "--max-time", "8", &url])
        .output()
        .map_err(|e| format!("curl: {e}"))?;

    if !output.status.success() {
        return Err("curl request failed".into());
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if text.is_empty() || text.starts_with("Unknown topic") || text.starts_with("<!doctype") {
        return Err("no result".into());
    }

    let clean: String = text
        .lines()
        .filter(|l| !l.starts_with("#[cheat") && !l.starts_with("#[tldr:"))
        .collect::<Vec<_>>()
        .join("\n");

    let result = clean.trim().to_string();
    cache_save(cmd, &result);

    Ok(result)
}

fn describe_windows(cmd: &str) -> Option<String> {
    let output = Command::new("cmd")
        .args(["/c", &format!("help {} 2>nul", cmd)])
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }

    None
}

fn is_command_available(name: &str) -> bool {
    if is_windows() {
        return Command::new("where")
            .arg(name)
            .output()
            .is_ok_and(|o| o.status.success());
    }

    Command::new("command")
        .args(["-v", name])
        .output()
        .is_ok_and(|o| o.status.success())
}

fn open_man_page(cmd: &str) {
    if !is_windows() {
        let _ = Command::new("man").arg(cmd).status();
    }
}

fn find_suggestions(cmd: &str) -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();

    if !is_windows()
        && let Ok(results) = run_cmd("apropos", &[cmd])
    {
        for line in results.lines() {
            if let Some(name) = line.split_whitespace().next() {
                let cleaned = name.trim_matches(':');
                if cmd.len() >= 2 && cleaned.contains(&cmd.to_lowercase()) {
                    candidates.push(cleaned.to_string());
                }
            }
        }
    }

    let builtin: Vec<String> = COMMON_COMMANDS.iter().map(|s| s.to_string()).collect();
    let fuzzy = fuzzy_find(cmd, &builtin, 5);
    for f in fuzzy {
        if !candidates.contains(&f) {
            candidates.push(f);
        }
    }

    candidates.truncate(8);
    candidates
}

fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    let sug_count = if app.suggestions.is_empty() {
        0u16
    } else {
        app.suggestions.len().min(5) as u16 + 1
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(3 + sug_count),
            Constraint::Min(2),
            Constraint::Length(3),
        ])
        .split(area);

    render_banner(f, chunks[0]);
    render_input(f, chunks[1], app);
    render_output(f, chunks[2], app);
    render_guide(f, chunks[3], app);
}

fn render_banner(f: &mut Frame, area: Rect) {
    let width = area.width as usize;
    let mut lines: Vec<Line> = Vec::new();

    for (i, line) in BANNER.iter().enumerate() {
        let color = BANNER_COLORS[i % BANNER_COLORS.len()];
        let text = if *line == "\u{2501}" {
            "\u{2501}".repeat(width)
        } else if i == 2 {
            format!("   LINUX COMMAND GUIDE  v{}   ", VERSION)
        } else {
            line.to_string()
        };

        let style = if color == Color::Black {
            Style::default()
        } else {
            Style::default().fg(color).add_modifier(Modifier::BOLD)
        };

        lines.push(Line::from(Span::styled(text, style)));
    }

    let para = Paragraph::new(Text::from(lines)).alignment(Alignment::Center);
    f.render_widget(para, area);
}

fn render_input(f: &mut Frame, area: Rect, app: &App) {
    let mut lines = Vec::new();

    let prompt = Span::styled(
        "\u{276F}\u{276F} ",
        Style::default()
            .fg(Color::Rgb(147, 112, 219))
            .add_modifier(Modifier::BOLD),
    );

    let ghost = if app.suggestions.is_empty() {
        String::new()
    } else {
        let sug = &app.suggestions[0];
        if sug.len() > app.input.len() {
            sug[app.input.len()..].to_string()
        } else {
            String::new()
        }
    };

    lines.push(Line::from(vec![
        prompt,
        Span::raw(&app.input),
        Span::styled(ghost, Style::default().fg(Color::DarkGray)),
    ]));

    if !app.suggestions.is_empty() {
        lines.push(Line::from(""));
        for (i, sug) in app.suggestions.iter().enumerate().take(5) {
            let sel = app.selected_suggestion == Some(i);
            let p = if sel { "\u{25B6} " } else { "  " };
            let st = if sel {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(147, 112, 219))
            };
            lines.push(Line::from(Span::styled(format!("{}{}", p, sug), st)));
        }
    }

    let para = Paragraph::new(Text::from(lines));
    f.render_widget(para, area);

    let cx = area.x + 3 + app.input.len() as u16;
    let cy = area.y;
    f.set_cursor_position((cx.min(area.x + area.width.saturating_sub(1)), cy));
}

fn render_output(f: &mut Frame, area: Rect, app: &App) {
    let lines: Vec<Line> = app
        .output
        .iter()
        .map(|l| {
            let (color, modifier) = if (l.starts_with('[') && l.ends_with(']'))
                || l == "   Arguments:"
                || l.contains("\u{2192}")
                || l.starts_with("  Welcome to Linux Guide")
            {
                (Color::Rgb(147, 112, 219), Modifier::BOLD)
            } else if l.starts_with("   Source:") {
                (Color::Cyan, Modifier::empty())
            } else if l.starts_with("   # ") || l == "   #" || l.starts_with("   #[") {
                (Color::DarkGray, Modifier::empty())
            } else if l.starts_with("     -") || l.starts_with("     --") {
                (Color::Yellow, Modifier::empty())
            } else {
                (Color::White, Modifier::empty())
            };
            Line::from(Span::styled(
                l,
                Style::default().fg(color).add_modifier(modifier),
            ))
        })
        .collect();

    let para = Paragraph::new(Text::from(lines))
        .scroll((app.scroll, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(para, area);
}

fn render_guide(f: &mut Frame, area: Rect, app: &App) {
    let sep = Span::styled(
        "\u{2500}".repeat(area.width as usize),
        Style::default().fg(Color::Rgb(147, 112, 219)),
    );

    let mut keys: Vec<(&str, Color)> = vec![
        (" Enter ", Color::Rgb(147, 112, 219)),
        ("Look up", Color::White),
        ("  \u{2502}  ", Color::DarkGray),
    ];

    if !app.suggestions.is_empty() {
        keys.extend_from_slice(&[
            (" Tab ", Color::Yellow),
            ("Complete", Color::White),
            ("  \u{2502}  ", Color::DarkGray),
        ]);
    }

    if app.show_man_prompt {
        keys.extend_from_slice(&[
            (" M ", Color::Rgb(147, 112, 219)),
            ("Man page", Color::White),
            ("  \u{2502}  ", Color::DarkGray),
        ]);
    }

    keys.extend_from_slice(&[
        (" \u{2191}\u{2193} ", Color::Rgb(147, 112, 219)),
        ("Scroll", Color::White),
        ("  \u{2502}  ", Color::DarkGray),
        (" Q ", Color::Red),
        ("Quit", Color::White),
    ]);

    let spans: Vec<Span> = keys
        .chunks(2)
        .flat_map(|pair| {
            if pair.len() == 2 {
                vec![
                    Span::styled(
                        pair[0].0,
                        Style::default().fg(pair[0].1).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(pair[1].0, Style::default().fg(pair[1].1)),
                ]
            } else {
                vec![Span::styled(pair[0].0, Style::default().fg(pair[0].1))]
            }
        })
        .collect();

    let para = Paragraph::new(Text::from(vec![Line::from(sep), Line::from(spans)]))
        .alignment(Alignment::Center);

    f.render_widget(para, area);
}

fn run_cmd(cmd: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("{}: {}", cmd, e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            "no output".into()
        } else {
            stderr
        })
    }
}

fn fuzzy_find(input: &str, candidates: &[String], max: usize) -> Vec<String> {
    let input = input.to_lowercase();
    let mut scored: Vec<(usize, &String)> = candidates
        .iter()
        .map(|c| (levenshtein(&input, &c.to_lowercase()), c))
        .collect();

    scored.sort_by_key(|(d, _)| *d);

    scored
        .into_iter()
        .filter(|(d, _)| *d <= 3)
        .take(max)
        .map(|(_, c)| c.clone())
        .collect()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

const COMMON_COMMANDS: &[&str] = &[
    "alias",
    "apropos",
    "apt",
    "arch",
    "arp",
    "at",
    "awk",
    "bash",
    "bc",
    "bg",
    "blkid",
    "btrfs",
    "cal",
    "cat",
    "cd",
    "chgrp",
    "chmod",
    "chown",
    "chroot",
    "cksum",
    "clear",
    "cmp",
    "cp",
    "cpio",
    "cron",
    "crontab",
    "curl",
    "cut",
    "date",
    "dd",
    "df",
    "diff",
    "dig",
    "dir",
    "dirname",
    "dmesg",
    "dpkg",
    "du",
    "echo",
    "ed",
    "env",
    "ethtool",
    "exit",
    "expand",
    "export",
    "expr",
    "factor",
    "false",
    "fc",
    "fg",
    "file",
    "find",
    "fmt",
    "fold",
    "free",
    "fsck",
    "fuser",
    "gawk",
    "git",
    "grep",
    "groupadd",
    "groupdel",
    "groupmod",
    "groups",
    "gzip",
    "halt",
    "hash",
    "head",
    "help",
    "history",
    "host",
    "hostname",
    "htop",
    "iconv",
    "id",
    "ifconfig",
    "info",
    "init",
    "iostat",
    "ip",
    "iptables",
    "jobs",
    "join",
    "journalctl",
    "kill",
    "killall",
    "less",
    "let",
    "link",
    "ln",
    "locate",
    "login",
    "logrotate",
    "look",
    "lscpu",
    "ls",
    "lsblk",
    "lshw",
    "lsmod",
    "lsof",
    "lspci",
    "lsscsi",
    "lsusb",
    "make",
    "man",
    "md5sum",
    "mkdir",
    "mkfifo",
    "mknod",
    "mkswap",
    "mktemp",
    "modprobe",
    "more",
    "mount",
    "mv",
    "nano",
    "nc",
    "netstat",
    "nice",
    "nl",
    "nmap",
    "nohup",
    "od",
    "openssl",
    "parted",
    "passwd",
    "paste",
    "patch",
    "perl",
    "pgrep",
    "pidof",
    "ping",
    "pkill",
    "poweroff",
    "printf",
    "pr",
    "ps",
    "pwd",
    "python3",
    "python",
    "read",
    "readlink",
    "realpath",
    "reboot",
    "rename",
    "renice",
    "reset",
    "rev",
    "rm",
    "rmdir",
    "route",
    "rpm",
    "rsync",
    "scp",
    "sed",
    "seq",
    "sftp",
    "sha1sum",
    "sha256sum",
    "shutdown",
    "sleep",
    "sort",
    "split",
    "ssh",
    "stat",
    "strace",
    "strings",
    "su",
    "sudo",
    "sum",
    "sync",
    "systemctl",
    "tac",
    "tail",
    "tar",
    "tee",
    "test",
    "time",
    "timeout",
    "tmux",
    "top",
    "touch",
    "tr",
    "traceroute",
    "tree",
    "true",
    "truncate",
    "tty",
    "type",
    "ulimit",
    "umask",
    "umount",
    "unalias",
    "uname",
    "uniq",
    "unlink",
    "unzip",
    "updatedb",
    "uptime",
    "useradd",
    "userdel",
    "usermod",
    "users",
    "vi",
    "vim",
    "vmstat",
    "w",
    "wall",
    "watch",
    "wc",
    "wget",
    "whatis",
    "whereis",
    "which",
    "who",
    "whoami",
    "xargs",
    "xz",
    "yes",
    "zcat",
    "zgrep",
    "zip",
    "zsh",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_identical() {
        assert_eq!(levenshtein("ls", "ls"), 0);
    }

    #[test]
    fn test_levenshtein_one_edit() {
        assert_eq!(levenshtein("lss", "ls"), 1);
    }

    #[test]
    fn test_fuzzy_find_typo() {
        let c: Vec<String> = ["ls", "lsof", "less"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let r = fuzzy_find("lss", &c, 3);
        assert!(r.contains(&"ls".to_string()));
    }

    #[test]
    fn test_describe_invalid() {
        assert!(describe_command("nonexistent_xyz_123").is_none());
    }
}
