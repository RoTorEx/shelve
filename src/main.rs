use crossterm::{
    ExecutableCommand, cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use inquire::Confirm;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use serde::Deserialize;
use std::{
    collections::HashSet,
    env, fs,
    io::stdout,
    path::{Path, PathBuf},
    process::{self, Command},
};

mod update;

#[derive(Deserialize)]
struct Config {
    #[serde(default)]
    inboxes: Vec<String>,
    locations: Vec<Location>,
}

#[derive(Clone, Deserialize)]
struct Location {
    group: String,
    label: String,
    path: String,
    #[serde(default)]
    move_here: bool,
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} / {}", self.group, self.label)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PickerEntry {
    Spacer,
    Group(String),
    Location(usize),
}

struct LocationPicker {
    locations: Vec<Location>,
    entries: Vec<PickerEntry>,
    selectable: Vec<usize>,
    selected: usize,
    list_offset: usize,
}

impl LocationPicker {
    fn new(locations: Vec<Location>) -> Self {
        let entries = picker_entries(&locations);
        let selectable = entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| matches!(entry, PickerEntry::Location(_)).then_some(index))
            .collect();
        Self {
            locations,
            entries,
            selectable,
            selected: 0,
            list_offset: 0,
        }
    }

    fn move_selection(&mut self, delta: isize) {
        self.selected = bounded_selection(self.selected, self.selectable.len(), delta);
    }

    fn selected_location(&self) -> Option<&Location> {
        let entry_index = *self.selectable.get(self.selected)?;
        let PickerEntry::Location(location_index) = self.entries.get(entry_index)? else {
            return None;
        };
        self.locations.get(*location_index)
    }
}

struct MovePlan {
    source: PathBuf,
    destination: PathBuf,
    ready: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("shelve: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("open") => open_location(load_config()?),
        Some("move") => move_files(load_config()?, args.collect()),
        Some("update") => update::run().map_err(|error| error.to_string()),
        Some("--version" | "-V") => {
            println!("shelve {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--help" | "-h") | None => {
            print_help();
            Ok(())
        }
        Some(command) => Err(format!("unknown command: {command}\n\nRun: shelve --help")),
    }
}

fn print_help() {
    println!(
        "shelve {}\n\nUsage:\n  shelve open\n  shelve move [FILE_OR_DIRECTORY ...]\n  shelve update\n\nCommands:\n  open    Choose a configured folder and open it in Finder\n  move    Choose destinations, preview, and move PDFs\n  update  Install the latest compatible GitHub Release\n\nOptions:\n  -h, --help     Print help\n  -V, --version  Print version",
        env!("CARGO_PKG_VERSION")
    );
}

fn config_path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("SHELVE_CONFIG") {
        return Ok(PathBuf::from(path));
    }
    if let Some(root) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(root).join("shelve/config.toml"));
    }
    env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".config/shelve/config.toml"))
        .ok_or_else(|| "HOME is not set".to_string())
}

fn expand_home(path: &str) -> Result<PathBuf, String> {
    if path == "~" || path.starts_with("~/") {
        let home = env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string())?;
        return Ok(PathBuf::from(home).join(path.trim_start_matches("~/")));
    }
    Ok(PathBuf::from(path))
}

fn load_config() -> Result<Config, String> {
    let path = config_path()?;
    let text = fs::read_to_string(&path).map_err(|_| {
        format!(
            "missing config {} (run `make install-local` from the repository)",
            path.display()
        )
    })?;
    let config: Config = toml::from_str(&text)
        .map_err(|error| format!("invalid config {}: {error}", path.display()))?;
    if config.locations.is_empty() {
        return Err("config has no locations".into());
    }
    Ok(config)
}

fn open_location(config: Config) -> Result<(), String> {
    let Some(location) = choose_location("Open folder", config.locations)? else {
        println!("Cancelled.");
        return Ok(());
    };
    let path = expand_home(&location.path)?;
    if !path.is_dir() {
        return Err(format!("folder does not exist: {}", path.display()));
    }
    let status = Command::new("open")
        .arg(&path)
        .status()
        .map_err(|error| format!("cannot start Finder: {error}"))?;
    if !status.success() {
        return Err(format!("Finder could not open {}", path.display()));
    }
    Ok(())
}

fn move_files(config: Config, inputs: Vec<String>) -> Result<(), String> {
    let destinations: Vec<Location> = config
        .locations
        .iter()
        .filter(|location| location.move_here)
        .cloned()
        .collect();
    if destinations.is_empty() {
        return Err("config has no locations with move_here = true".into());
    }

    let paths: Vec<&str> = if inputs.is_empty() {
        config.inboxes.iter().map(String::as_str).collect()
    } else {
        inputs.iter().map(String::as_str).collect()
    };
    let sources = collect_pdfs(&paths)?;
    if sources.is_empty() {
        println!("No PDFs found.");
        return Ok(());
    }

    let mut plans = Vec::new();
    for source in sources {
        let prompt = format!(
            "Move {}",
            source.file_name().unwrap_or_default().to_string_lossy()
        );
        let Some(location) = choose_location(&prompt, destinations.clone())? else {
            println!("Cancelled.");
            return Ok(());
        };
        let directory = expand_home(&location.path)?;
        if !directory.is_dir() {
            return Err(format!("folder does not exist: {}", directory.display()));
        }
        let destination = directory.join(
            source
                .file_name()
                .ok_or_else(|| "source has no filename".to_string())?,
        );
        let ready = source != destination && !destination.exists();
        plans.push(MovePlan {
            source,
            destination,
            ready,
        });
    }

    println!("\nPreview:");
    for plan in &plans {
        let status = if plan.ready {
            "ready"
        } else if plan.source == plan.destination {
            "already there"
        } else {
            "destination exists"
        };
        println!(
            "  [{status}] {} -> {}",
            plan.source.display(),
            plan.destination.display()
        );
    }

    let ready = plans.iter().filter(|plan| plan.ready).count();
    if ready == 0 {
        println!("Nothing to move.");
        return Ok(());
    }
    if !Confirm::new(&format!("Move {ready} file(s)?"))
        .with_default(false)
        .prompt()
        .map_err(|error| error.to_string())?
    {
        println!("Cancelled.");
        return Ok(());
    }

    let mut moved = 0;
    let mut failed = 0;
    for plan in plans.into_iter().filter(|plan| plan.ready) {
        match move_without_overwrite(&plan.source, &plan.destination) {
            Ok(()) => {
                moved += 1;
                println!("moved: {}", plan.destination.display());
            }
            Err(error) => {
                failed += 1;
                eprintln!("failed: {}: {error}", plan.source.display());
            }
        }
    }
    println!("Done: {moved} moved, {failed} failed.");
    if failed == 0 {
        Ok(())
    } else {
        Err("one or more files could not be moved".into())
    }
}

fn picker_entries(locations: &[Location]) -> Vec<PickerEntry> {
    let mut entries = Vec::new();
    let mut current_group: Option<&str> = None;

    for (index, location) in locations.iter().enumerate() {
        if current_group != Some(location.group.as_str()) {
            if current_group.is_some() {
                entries.push(PickerEntry::Spacer);
            }
            current_group = Some(&location.group);
            entries.push(PickerEntry::Group(location.group.clone()));
        }
        entries.push(PickerEntry::Location(index));
    }

    entries
}

fn bounded_selection(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    if delta < 0 {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        current.saturating_add(delta.unsigned_abs()).min(len - 1)
    }
}

fn choose_location(title: &str, locations: Vec<Location>) -> Result<Option<Location>, String> {
    if locations.is_empty() {
        return Err("no locations available".into());
    }

    let mut picker = LocationPicker::new(locations);
    enable_raw_mode().map_err(|error| format!("cannot start interface: {error}"))?;
    let _guard = PickerTerminalGuard;
    let mut output = stdout();
    output
        .execute(EnterAlternateScreen)
        .and_then(|output| output.execute(cursor::Hide))
        .map_err(|error| format!("cannot start interface: {error}"))?;
    let backend = CrosstermBackend::new(output);
    let mut terminal =
        Terminal::new(backend).map_err(|error| format!("cannot start interface: {error}"))?;

    loop {
        terminal
            .draw(|frame| draw_location_picker(frame, &mut picker, title))
            .map_err(|error| format!("cannot draw interface: {error}"))?;

        let Event::Key(key) =
            event::read().map_err(|error| format!("cannot read keyboard input: {error}"))?
        else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Up => picker.move_selection(-1),
            KeyCode::Down => picker.move_selection(1),
            KeyCode::Home => picker.selected = 0,
            KeyCode::End => picker.selected = picker.selectable.len().saturating_sub(1),
            KeyCode::Enter => return Ok(picker.selected_location().cloned()),
            KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(None),
            KeyCode::Char('k') if key.modifiers.is_empty() => picker.move_selection(-1),
            KeyCode::Char('j') if key.modifiers.is_empty() => picker.move_selection(1),
            _ => {}
        }
    }
}

struct PickerTerminalGuard;

impl Drop for PickerTerminalGuard {
    fn drop(&mut self) {
        let mut output = stdout();
        let _ = output.execute(cursor::Show);
        let _ = output.execute(LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

fn draw_location_picker(frame: &mut ratatui::Frame, picker: &mut LocationPicker, title: &str) {
    let area = frame.area();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);
    let body = if area.width >= 72 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
            .split(rows[0])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(7)])
            .split(rows[0])
    };

    draw_location_list(frame, picker, body[0], title);
    draw_location_detail(frame, picker, body[1]);

    let position = if picker.selectable.is_empty() {
        "0/0".to_string()
    } else {
        format!("{}/{}", picker.selected + 1, picker.selectable.len())
    };
    let status = Line::from(vec![
        Span::styled(" ↑/↓ ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("navigate  "),
        Span::styled("Enter ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("select  "),
        Span::styled("Esc ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("cancel"),
        Span::styled(format!("  {position} "), Style::default().fg(Color::Gray)),
    ]);
    frame.render_widget(Paragraph::new(status).alignment(Alignment::Center), rows[1]);
}

fn draw_location_list(
    frame: &mut ratatui::Frame,
    picker: &mut LocationPicker,
    area: Rect,
    title: &str,
) {
    let block = Block::default()
        .title(format!(" {title} "))
        .title_style(Style::default().add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = picker
        .entries
        .iter()
        .map(|entry| match entry {
            PickerEntry::Spacer => ListItem::new(Line::default()),
            PickerEntry::Group(group) => ListItem::new(Line::from(vec![
                Span::raw("◆ "),
                Span::styled(
                    group.as_str(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ])),
            PickerEntry::Location(index) => {
                ListItem::new(Line::from(format!("  {}", picker.locations[*index].label)))
            }
        })
        .collect();
    let list = List::new(items)
        .highlight_symbol("")
        .highlight_style(Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED))
        .scroll_padding(2);
    let selected_entry = picker.selectable.get(picker.selected).copied();
    let mut state = ListState::default()
        .with_offset(picker.list_offset)
        .with_selected(selected_entry);
    frame.render_stateful_widget(list, inner, &mut state);
    picker.list_offset = state.offset();
}

fn draw_location_detail(frame: &mut ratatui::Frame, picker: &LocationPicker, area: Rect) {
    let block = Block::default()
        .title(" Destination ")
        .title_style(Style::default().add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(location) = picker.selected_location() else {
        return;
    };
    let detail = Text::from(vec![
        Line::from(Span::styled(
            location.group.as_str(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            location.label.as_str(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::default(),
        Line::from(location.path.as_str()),
    ]);
    frame.render_widget(Paragraph::new(detail).wrap(Wrap { trim: false }), inner);
}

fn collect_pdfs(inputs: &[&str]) -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    for input in inputs {
        let path = expand_home(input)?;
        if path.is_dir() {
            let entries = fs::read_dir(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            for entry in entries {
                let entry =
                    entry.map_err(|error| format!("cannot read {}: {error}", path.display()))?;
                let candidate = entry.path();
                if candidate.is_file() && is_pdf(&candidate) {
                    add_source(candidate, &mut found, &mut seen)?;
                }
            }
        } else if path.is_file() && is_pdf(&path) {
            add_source(path, &mut found, &mut seen)?;
        } else {
            eprintln!("skipped: {} (not a PDF file or directory)", path.display());
        }
    }
    found.sort();
    Ok(found)
}

fn add_source(
    path: PathBuf,
    found: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if seen.insert(canonical.clone()) {
        found.push(canonical);
    }
    Ok(())
}

fn is_pdf(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
}

fn move_without_overwrite(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err(format!("destination exists: {}", destination.display()));
    }
    fs::hard_link(source, destination).map_err(|error| {
        format!("cannot create destination (files must be on the same filesystem): {error}")
    })?;
    if let Err(error) = fs::remove_file(source) {
        let _ = fs::remove_file(destination);
        return Err(format!(
            "destination created but source could not be removed: {error}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_extension_is_case_insensitive() {
        assert!(is_pdf(Path::new("invoice.PDF")));
        assert!(!is_pdf(Path::new("invoice.txt")));
    }

    #[test]
    fn move_does_not_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.pdf");
        let destination = temp.path().join("destination.pdf");
        fs::write(&source, "source").unwrap();
        fs::write(&destination, "existing").unwrap();
        assert!(move_without_overwrite(&source, &destination).is_err());
        assert_eq!(fs::read_to_string(&source).unwrap(), "source");
        assert_eq!(fs::read_to_string(&destination).unwrap(), "existing");
    }

    #[test]
    fn move_removes_source() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.pdf");
        let destination = temp.path().join("destination.pdf");
        fs::write(&source, "content").unwrap();
        move_without_overwrite(&source, &destination).unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read_to_string(destination).unwrap(), "content");
    }

    #[test]
    fn selection_stops_at_list_boundaries() {
        assert_eq!(bounded_selection(0, 3, -1), 0);
        assert_eq!(bounded_selection(2, 3, 1), 2);
        assert_eq!(bounded_selection(1, 3, -1), 0);
        assert_eq!(bounded_selection(1, 3, 1), 2);
    }

    #[test]
    fn picker_separates_location_groups() {
        let locations = vec![
            Location {
                group: "Money In".into(),
                label: "Invoices".into(),
                path: "/invoices".into(),
                move_here: true,
            },
            Location {
                group: "Money In".into(),
                label: "Payments".into(),
                path: "/payments".into(),
                move_here: true,
            },
            Location {
                group: "Money Out".into(),
                label: "Taxes".into(),
                path: "/taxes".into(),
                move_here: true,
            },
        ];

        assert_eq!(
            picker_entries(&locations),
            vec![
                PickerEntry::Group("Money In".into()),
                PickerEntry::Location(0),
                PickerEntry::Location(1),
                PickerEntry::Spacer,
                PickerEntry::Group("Money Out".into()),
                PickerEntry::Location(2),
            ]
        );
    }

    #[test]
    fn picker_renders_separate_list_and_detail_blocks() {
        let locations = vec![Location {
            group: "Money In".into(),
            label: "Invoices".into(),
            path: "/shelf/invoices".into(),
            move_here: true,
        }];
        let mut picker = LocationPicker::new(locations);
        let backend = ratatui::backend::TestBackend::new(90, 14);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| draw_location_picker(frame, &mut picker, "Open folder"))
            .unwrap();

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("Open folder"));
        assert!(rendered.contains("Destination"));
        assert!(rendered.contains("Money In"));
        assert!(rendered.contains("/shelf/invoices"));
        assert!(rendered.contains("navigate"));

        let cells = terminal.backend().buffer().content();
        assert!(
            cells
                .iter()
                .any(|cell| cell.modifier.contains(Modifier::REVERSED))
        );
        assert!(cells.iter().all(|cell| {
            !matches!(cell.fg, Color::Cyan | Color::DarkGray)
                && !matches!(cell.bg, Color::Cyan | Color::DarkGray)
        }));
    }

    #[test]
    fn starter_config_is_universal_and_supports_both_commands() {
        let config: Config = toml::from_str(include_str!("../config.example.toml")).unwrap();
        let paths: Vec<&str> = config
            .locations
            .iter()
            .map(|location| location.path.as_str())
            .collect();

        assert_eq!(paths, vec!["~", "~/Desktop", "~/Downloads", "~/Documents"]);
        assert!(config.locations.iter().any(|location| location.move_here));
    }
}
