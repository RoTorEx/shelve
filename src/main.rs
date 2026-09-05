use inquire::Confirm;
use serde::Deserialize;
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    process::{self, Command},
};

mod picker;
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
        Some("open") => {
            let selector = args.next();
            if args.next().is_some() {
                return Err("usage: shelve open [SELECTOR]".into());
            }
            open_location(load_config()?, selector.as_deref())
        }
        Some("move") => move_files(load_config()?, args.collect()),
        Some("update") => update::run().map_err(|error| error.to_string()),
        Some("--version" | "-V") => {
            println!("shelve {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        None => open_location(load_config()?, None),
        Some("--help" | "-h") => {
            print_help();
            Ok(())
        }
        Some(command) => Err(format!("unknown command: {command}\n\nRun: shelve --help")),
    }
}

fn print_help() {
    println!(
        "shelve {}\n\nUsage:\n  shelve\n  shelve open [SELECTOR]\n  shelve move [FILE_OR_DIRECTORY ...]\n  shelve update\n\nCommands:\n  open    Choose a configured folder and open it in Finder\n  move    Choose destinations, preview, and move PDFs\n  update  Install the latest compatible GitHub Release\n\nOptions:\n  -h, --help     Print help\n  -V, --version  Print version",
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

fn open_location(config: Config, selector: Option<&str>) -> Result<(), String> {
    let location = if let Some(selector) = selector {
        Some(picker::resolve(&config.locations, selector, false)?)
    } else {
        picker::choose("Open folder", &config.locations, false)?
    };
    let Some(location) = location else {
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
    if !config.locations.iter().any(|location| location.move_here) {
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
        let Some(location) = picker::choose(&prompt, &config.locations, true)? else {
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
