use crate::Location;
use std::{
    io::{self, BufRead, IsTerminal, Write},
    path::Path,
};

struct Group<'a> {
    name: &'a str,
    locations: Vec<&'a Location>,
}

fn groups(locations: &[Location]) -> Vec<Group<'_>> {
    let mut groups: Vec<Group<'_>> = Vec::new();
    for location in locations {
        if let Some(group) = groups.iter_mut().find(|group| group.name == location.group) {
            group.locations.push(location);
        } else {
            groups.push(Group {
                name: &location.group,
                locations: vec![location],
            });
        }
    }
    groups
}

fn group_label(mut index: usize) -> String {
    let mut label = Vec::new();
    loop {
        label.push(b'A' + (index % 26) as u8);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    label.reverse();
    String::from_utf8(label).expect("ASCII group label")
}

pub(crate) fn resolve(
    locations: &[Location],
    input: &str,
    move_only: bool,
) -> Result<Location, String> {
    let choice = input.trim();
    let split = choice
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(choice.len());
    let (label, number) = choice.split_at(split);
    let number = number.trim();
    if label.is_empty() || number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return Err("use a group letter and folder number, for example A1".into());
    }
    let number = number
        .parse::<usize>()
        .map_err(|_| format!("invalid position: {number}"))?;
    let grouped = groups(locations);
    let group = grouped
        .iter()
        .enumerate()
        .find(|(i, _)| group_label(*i).eq_ignore_ascii_case(label))
        .map(|(_, group)| group)
        .ok_or_else(|| format!("unknown destination: {choice}"))?;
    if number == 0 {
        if move_only {
            return Err("group roots are for opening, not PDF moves".into());
        }
        let root = shared_parent(group).ok_or_else(|| "group has no common root".to_string())?;
        return Ok(Location {
            group: group.name.into(),
            label: folder_name(root),
            path: root.to_string_lossy().into_owned(),
            move_here: false,
        });
    }
    let location = group
        .locations
        .get(number - 1)
        .copied()
        .ok_or_else(|| format!("unknown destination: {choice}"))?;
    if move_only && !location.move_here {
        return Err(format!("{choice} is not a PDF move destination"));
    }
    Ok(location.clone())
}

fn paint(color: bool, code: &str, text: &str) -> String {
    if color {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.into()
    }
}

fn folder_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

fn shared_parent<'a>(group: &Group<'a>) -> Option<&'a Path> {
    let first = Path::new(&group.locations.first()?.path);
    let mut base = if group.locations.len() == 1 {
        first.parent()?
    } else {
        first
    };
    for location in &group.locations {
        while !Path::new(&location.path).starts_with(base) {
            base = base.parent()?;
        }
    }
    (!base.as_os_str().is_empty()).then_some(base)
}

fn render(
    out: &mut impl Write,
    title: &str,
    locations: &[Location],
    move_only: bool,
    color: bool,
) -> io::Result<()> {
    writeln!(
        out,
        "\n  {} {} {}",
        paint(color, "33", "*"),
        paint(color, "1", "Shelve"),
        paint(color, "2", &format!("(v{})", env!("CARGO_PKG_VERSION")))
    )?;
    writeln!(
        out,
        "\n  {} {} {}",
        paint(color, "2", "--------------"),
        paint(color, "34", title),
        paint(color, "2", "--------------")
    )?;
    for (index, group) in groups(locations).iter().enumerate() {
        if !group
            .locations
            .iter()
            .any(|location| !move_only || location.move_here)
        {
            continue;
        }
        let parent = shared_parent(group);
        let context = parent
            .map(|root| {
                root.parent()
                    .filter(|path| !path.as_os_str().is_empty())
                    .unwrap_or(root)
            })
            .map(|path| format!(" ({}/)", path.display().to_string().trim_end_matches('/')))
            .unwrap_or_default();
        writeln!(
            out,
            "\n  {}.  {}{}",
            paint(color, "33", &group_label(index)),
            paint(
                color,
                "1",
                &parent.map(folder_name).unwrap_or_else(|| group.name.into())
            ),
            paint(color, "90", &context)
        )?;
        for (index, location) in group.locations.iter().enumerate() {
            if !move_only || location.move_here {
                let path = match parent {
                    Some(parent) => Path::new(&location.path)
                        .strip_prefix(parent)
                        .unwrap_or(Path::new(&location.path)),
                    None => Path::new(&location.path),
                };
                let name = folder_name(Path::new(&location.path));
                let context = if parent.is_some() && path.to_string_lossy() == name {
                    String::new()
                } else {
                    format!(
                        " ({})",
                        if path.as_os_str().is_empty() {
                            ".".into()
                        } else {
                            path.display().to_string()
                        }
                    )
                };
                writeln!(
                    out,
                    "     {} {}{}",
                    paint(color, "36", &format!("{:>2})", index + 1)),
                    paint(color, "32", &name),
                    paint(color, "90", &context)
                )?;
            }
        }
    }
    writeln!(out)
}

fn prompt(
    input: &mut impl BufRead,
    out: &mut impl Write,
    locations: &[Location],
    move_only: bool,
) -> Result<Option<Location>, String> {
    loop {
        write!(out, "  > open <sector><position>: ")
            .and_then(|_| out.flush())
            .map_err(|e| e.to_string())?;
        let mut line = String::new();
        input
            .read_line(&mut line)
            .map_err(|e| format!("cannot read selection: {e}"))?;
        let choice = line.trim();
        if choice.is_empty() || choice.eq_ignore_ascii_case("q") || choice == "\u{1b}" {
            return Ok(None);
        }
        match resolve(locations, choice, move_only) {
            Ok(location) => return Ok(Some(location)),
            Err(error) => writeln!(out, "\n  {error}\n").map_err(|e| e.to_string())?,
        }
    }
}

pub(crate) fn choose(
    title: &str,
    locations: &[Location],
    move_only: bool,
) -> Result<Option<Location>, String> {
    let color = io::stderr().is_terminal()
        && std::env::var_os("NO_COLOR").is_none()
        && std::env::var("TERM").is_ok_and(|term| term != "dumb");
    let mut out = io::stderr().lock();
    render(&mut out, title, locations, move_only, color).map_err(|e| e.to_string())?;
    prompt(&mut io::stdin().lock(), &mut out, locations, move_only)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locations() -> Vec<Location> {
        [
            ("Home", "Desktop", false),
            ("Business", "Root", false),
            ("Business", "Invoices", true),
            ("Home", "Downloads", false),
        ]
        .into_iter()
        .map(|(group, label, move_here)| Location {
            group: group.into(),
            label: label.into(),
            path: format!("/{label}"),
            move_here,
        })
        .collect()
    }

    #[test]
    fn selectors_group_nonadjacent_entries_and_accept_lowercase() {
        let locations = locations();
        assert_eq!(
            resolve(&locations, " a2 ", false).unwrap().label,
            "Downloads"
        );
        assert_eq!(resolve(&locations, "B 2", false).unwrap().label, "Invoices");
        for choice in [
            "",
            "A",
            "1",
            "A3",
            "Z1",
            "A-1",
            "A1x",
            "А1",
            "A999999999999999999999999999999",
        ] {
            assert!(resolve(&locations, choice, false).is_err(), "{choice}");
        }
    }

    #[test]
    fn move_menu_preserves_codes_and_rejects_open_only_destinations() {
        let locations = locations();
        assert_eq!(resolve(&locations, "B2", true).unwrap().label, "Invoices");
        assert!(resolve(&locations, "B1", true).is_err());
        let mut out = Vec::new();
        render(&mut out, "Move PDF", &locations, true, false).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("\n\n  B.  / (/)\n      2) Invoices"));
        assert!(!text.contains("Desktop"));
        assert!(!text.contains("\x1b["));
    }

    #[test]
    fn zero_opens_the_group_root_and_names_come_from_paths() {
        let locations = vec![Location {
            group: "Custom group".into(),
            label: "Custom alias".into(),
            path: "~/Documents/Business/In Invoices".into(),
            move_here: true,
        }];
        assert_eq!(
            resolve(&locations, "a0", false).unwrap().path,
            "~/Documents/Business"
        );
        assert_eq!(
            resolve(&locations, "A1", false).unwrap().path,
            locations[0].path
        );
        assert!(resolve(&locations, "A0", true).is_err());
        let mut out = Vec::new();
        render(&mut out, "Open folder", &locations, false, false).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("A.  Business (~/Documents/)"));
        assert!(text.contains("1) In Invoices"));
        assert!(!text.contains("Custom"));
        assert!(!text.lines().any(|line| line.trim_start().starts_with("0)")));
    }

    #[test]
    fn supports_multiletter_groups_and_multidigit_positions() {
        assert_eq!(group_label(25), "Z");
        assert_eq!(group_label(26), "AA");
        let mut locations = locations();
        for i in 0..27 {
            locations.push(Location {
                group: format!("Group {i}"),
                label: "Folder".into(),
                path: "/folder".into(),
                move_here: false,
            });
        }
        assert_eq!(resolve(&locations, "AA1", false).unwrap().group, "Group 24");
        for _ in 0..10 {
            locations.push(locations[0].clone());
        }
        assert_eq!(resolve(&locations, "A12", false).unwrap().label, "Desktop");
    }

    #[test]
    fn invalid_input_retries_and_eof_or_empty_input_cancels() {
        let locations = locations();
        let mut out = Vec::new();
        let selected = prompt(&mut &b"Z8\nb2\n"[..], &mut out, &locations, false)
            .unwrap()
            .unwrap();
        assert_eq!(selected.label, "Invoices");
        assert!(
            String::from_utf8(out)
                .unwrap()
                .contains("unknown destination")
        );
        for input in ["", "\n", "q\n"] {
            assert!(
                prompt(&mut input.as_bytes(), &mut Vec::new(), &locations, false)
                    .unwrap()
                    .is_none()
            );
        }
    }
}
