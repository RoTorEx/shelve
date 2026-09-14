use inquire::Confirm;
use serde::Deserialize;
use std::{
    collections::HashSet,
    env, fs,
    path::{Component, Path, PathBuf},
    process::{self, Command},
};

mod picker;
mod update;

const CONFIG_VERSION: u32 = 2;

#[derive(Deserialize)]
struct ConfigFile {
    #[serde(default = "default_config_version")]
    version: u32,
    #[serde(default)]
    inboxes: Vec<String>,
    #[serde(default)]
    locations: Vec<Location>,
    #[serde(default)]
    sections: Vec<Section>,
}

struct Config {
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
    #[serde(skip)]
    root: Option<String>,
    #[serde(skip)]
    is_section_root: bool,
}

#[derive(Deserialize)]
struct Section {
    root: String,
    #[serde(default)]
    children: bool,
    #[serde(default)]
    pins: Vec<String>,
    #[serde(default)]
    move_here: Vec<String>,
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
        Some("--help" | "-h") | None => {
            print_help();
            Ok(())
        }
        Some(command) => Err(format!("unknown command: {command}\n\nRun: shelve --help")),
    }
}

fn print_help() {
    println!(
        "shelve {}\n\nUsage:\n  shelve open [SELECTOR]\n  shelve move [FILE_OR_DIRECTORY ...]\n  shelve update\n\nCommands:\n  open    Choose a configured folder and open it in Finder\n  move    Choose destinations, preview, and move PDFs\n  update  Install the latest compatible GitHub Release\n\nOptions:\n  -h, --help     Print help\n  -V, --version  Print version",
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

const fn default_config_version() -> u32 {
    1
}

fn expand_home(path: &str) -> Result<PathBuf, String> {
    if path == "~" {
        let home = env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string())?;
        return Ok(PathBuf::from(home));
    }
    if let Some(relative) = path.strip_prefix("~/") {
        let home = env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string())?;
        return Ok(PathBuf::from(home).join(relative));
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
    let config: ConfigFile = toml::from_str(&text)
        .map_err(|error| format!("invalid config {}: {error}", path.display()))?;
    materialize_config(config)
        .map_err(|error| format!("invalid config {}: {error}", path.display()))
}

fn materialize_config(config: ConfigFile) -> Result<Config, String> {
    if !(1..=CONFIG_VERSION).contains(&config.version) {
        return Err(format!("unsupported config version {}", config.version));
    }
    if config.version < 2 && !config.sections.is_empty() {
        return Err("sections require config version = 2".into());
    }

    let mut locations = config.locations;
    for section in config.sections {
        locations.extend(materialize_section(section)?);
    }
    if locations.is_empty() {
        return Err("config has no locations".into());
    }
    Ok(Config {
        inboxes: config.inboxes,
        locations,
    })
}

fn materialize_section(section: Section) -> Result<Vec<Location>, String> {
    let root = expand_home(&section.root)?;
    if !root.is_dir() {
        return Err(format!("section root is not a folder: {}", root.display()));
    }

    let mut paths = Vec::new();
    let mut seen = HashSet::new();
    for pin in &section.pins {
        let path = resolve_section_path(&root, pin)?;
        if path == root {
            continue;
        }
        if seen.insert(path.clone()) {
            paths.push(path);
        }
    }

    if section.children {
        let entries = fs::read_dir(&root)
            .map_err(|error| format!("cannot read section root {}: {error}", root.display()))?;
        let mut children = Vec::new();
        for entry in entries {
            let entry = entry
                .map_err(|error| format!("cannot read section root {}: {error}", root.display()))?;
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let kind = entry.file_type().map_err(|error| {
                format!(
                    "cannot inspect section child {}: {error}",
                    entry.path().display()
                )
            })?;
            if kind.is_dir() {
                children.push(entry.path());
            }
        }
        children.sort_by(|left, right| {
            left.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
                .cmp(
                    &right
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase(),
                )
                .then_with(|| left.cmp(right))
        });
        for child in children {
            if seen.insert(child.clone()) {
                paths.push(child);
            }
        }
    }

    let move_here = section
        .move_here
        .iter()
        .map(|path| resolve_section_path(&root, path))
        .collect::<Result<HashSet<_>, _>>()?;
    if let Some(path) = move_here.iter().find(|path| !seen.contains(*path)) {
        return Err(format!(
            "move_here path is not a pin or discovered child: {}",
            path.display()
        ));
    }

    let group = root.to_string_lossy().into_owned();
    let mut locations = vec![Location {
        group: group.clone(),
        label: root
            .file_name()
            .unwrap_or(root.as_os_str())
            .to_string_lossy()
            .into_owned(),
        path: group.clone(),
        move_here: false,
        root: Some(group.clone()),
        is_section_root: true,
    }];
    locations.extend(paths.into_iter().map(|path| {
        Location {
            group: group.clone(),
            label: path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned(),
            move_here: move_here.contains(&path),
            path: path.to_string_lossy().into_owned(),
            root: Some(group.clone()),
            is_section_root: false,
        }
    }));
    Ok(locations)
}

fn resolve_section_path(root: &Path, configured: &str) -> Result<PathBuf, String> {
    let path = Path::new(configured);
    let resolved = if path.is_absolute() || configured == "~" || configured.starts_with("~/") {
        expand_home(configured)?
    } else {
        if path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(format!(
                "relative section path must stay below its root: {configured}"
            ));
        }
        root.join(path)
    };
    if !resolved.starts_with(root) {
        return Err(format!(
            "section path is outside its root {}: {}",
            root.display(),
            resolved.display()
        ));
    }
    Ok(resolved)
}

fn open_location(config: Config, selector: Option<&str>) -> Result<(), String> {
    let location = if let Some(selector) = selector {
        Some(picker::resolve(&config.locations, selector, false)?)
    } else {
        picker::choose("Folders", &config.locations, false)?
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

    let Some(sources) = picker::choose_sources(&sources)? else {
        println!("Cancelled.");
        return Ok(());
    };

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
    fn home_expansion_handles_home_itself_and_descendants() {
        let home = PathBuf::from(env::var_os("HOME").unwrap());
        assert_eq!(expand_home("~").unwrap(), home);
        assert_eq!(expand_home("~/Documents").unwrap(), home.join("Documents"));
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
        let parsed: ConfigFile = toml::from_str(include_str!("../config.example.toml")).unwrap();
        let config = materialize_config(parsed).unwrap();
        let paths: Vec<&str> = config
            .locations
            .iter()
            .filter(|location| !location.is_section_root)
            .map(|location| location.path.as_str())
            .collect();

        let home = env::var("HOME").unwrap();
        assert_eq!(
            paths,
            vec![
                format!("{home}/Desktop"),
                format!("{home}/Downloads"),
                format!("{home}/Documents")
            ]
        );
        assert!(config.locations.iter().any(|location| location.move_here));
    }

    #[test]
    fn legacy_locations_remain_supported() {
        let parsed: ConfigFile = toml::from_str(
            r#"
inboxes = ["~/Downloads"]

[[locations]]
group = "Documents"
label = "Archive"
path = "~/Documents/Archive"
move_here = true
"#,
        )
        .unwrap();
        let config = materialize_config(parsed).unwrap();

        assert_eq!(config.locations.len(), 1);
        assert_eq!(config.locations[0].path, "~/Documents/Archive");
        assert_eq!(config.locations[0].root, None);
        assert!(!config.locations[0].is_section_root);
        assert!(config.locations[0].move_here);
    }

    #[test]
    fn sections_require_config_version_two() {
        let parsed: ConfigFile = toml::from_str(
            r#"
[[sections]]
root = "~"
pins = ["Documents"]
"#,
        )
        .unwrap();

        assert_eq!(
            materialize_config(parsed).err().as_deref(),
            Some("sections require config version = 2")
        );
    }

    #[test]
    fn section_combines_pins_at_any_depth_and_direct_children() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("Library");
        let books = root.join("Books");
        let guides = root.join("Guides");
        let english = guides.join("English");
        for path in [&books, &english, &root.join(".hidden")] {
            fs::create_dir_all(path).unwrap();
        }
        let section = Section {
            root: root.to_string_lossy().into_owned(),
            children: true,
            pins: vec!["Books".into(), "Guides/English".into()],
            move_here: vec!["Books".into()],
        };

        let locations = materialize_section(section).unwrap();
        let paths = locations
            .iter()
            .filter(|location| !location.is_section_root)
            .map(|location| PathBuf::from(&location.path))
            .collect::<Vec<_>>();
        assert_eq!(paths, vec![books, english, guides]);
        assert!(locations[1].move_here);
        assert_eq!(locations[0].root.as_deref(), root.to_str());
        assert!(locations[0].is_section_root);
    }

    #[test]
    fn section_rejects_a_pin_outside_its_root() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("Library");
        let outside = temp.path().join("Storage");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let section = Section {
            root: root.to_string_lossy().into_owned(),
            children: false,
            pins: vec![outside.to_string_lossy().into_owned()],
            move_here: Vec::new(),
        };

        let error = materialize_section(section).err().unwrap();
        assert!(error.contains("section path is outside its root"));
        assert!(error.contains(&outside.to_string_lossy().into_owned()));
    }

    #[test]
    fn section_rejects_move_destination_that_is_not_visible() {
        let temp = tempfile::tempdir().unwrap();
        let section = Section {
            root: temp.path().to_string_lossy().into_owned(),
            children: false,
            pins: Vec::new(),
            move_here: vec!["Invoices".into()],
        };

        assert!(materialize_section(section).is_err());
    }

    #[test]
    fn section_root_is_available_without_children_or_pins() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_string_lossy().into_owned();
        let section = Section {
            root: root.clone(),
            children: false,
            pins: Vec::new(),
            move_here: Vec::new(),
        };

        let locations = materialize_section(section).unwrap();
        assert_eq!(locations.len(), 1);
        assert!(locations[0].is_section_root);
        assert_eq!(picker::resolve(&locations, "A0", false).unwrap().path, root);
        assert!(picker::resolve(&locations, "A1", false).is_err());
    }
}
