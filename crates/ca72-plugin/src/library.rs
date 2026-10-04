//! The presets (decisions.md R10), in a library shared with the DAW the model was developed
//! in: a sound is named panel settings, the dials' plain values by parameter ID (a choice by
//! its index, a switch 0 or 1, POLY and VOICES too; never the PITCH wheel or the bypass).
//!
//! - **The factory's** are built in (`sounds/presets.toml`; the DAW carries its own copy) and
//!   never change. The file ships whole inside the plug-in, so it names no trademark (R1).
//! - **The user's** are files in the library's folder, `<name>.toml` (`format`, `name`,
//!   `description`, `tags`, `favorite`, `replaces`, `values`), written whole and moved into
//!   place so the other program never reads half of one. A user preset named as a factory
//!   one (or saying it `replaces` one) stands in for it: an edit or a rename of a factory
//!   preset, taken back by [`Library::revert`].
//! - **The factory's tags, favourites and deletions** are the folder's `factory.toml`
//!   (`[presets."<name>"]`: `tags`, `favorite`, `hidden`); [`Library::restore`] brings the
//!   deleted back.
//! - The folder is `<shared>/CA-72/Presets`, `<shared>` being `$IDLE_FOUNDRY_SHARED`, else the
//!   user's data folder's `Idle Foundry` (`~/.local/share`, `~/Library/Application Support`,
//!   `%APPDATA%`).
//!
//! Names are 1 to 64 characters, none of `/ \ : * ? " < > |`, not starting or ending with a
//! space or a dot, unique ignoring case; never `factory` (the folder's own file) nor a name
//! Windows keeps for a device (`CON`, `NUL`, `COM1` and the like, with any extension), on every
//! system, since the folder may be shared between them (decisions.md R18). A file whose `name`
//! breaks these is reported, not listed.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use toml::Value;

/// The format of the files this reads and writes.
pub const FORMAT: i64 = 1;

/// The parameters a preset never sets: the PITCH wheel (where the player leaves it) and the
/// bypass (the host's).
pub const KEPT: &[&str] = &["pitch_wheel", "bypass"];

const MOST_TAGS: usize = 16;
const LONGEST_TAG: usize = 32;

/// The folder's own file's name (the factory presets' tags, favourites and deletions).
const OVERLAY: &str = "factory";

/// The names Windows keeps for its devices, an extension or not (`NUL.txt` is `NUL`), and
/// those it adds a digit to (`COM0` to `COM9`, `COM¹` to `COM³`, `LPT` the same).
const DEVICES: &[&str] = &["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"];
const NUMBERED_DEVICES: &[&str] = &["COM", "LPT"];

/// A preset's sound: its values (plain) and what a browser shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub values: Vec<(String, f64)>,
}

/// Where a preset comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Factory,
    /// The user's version of a factory preset (edited or renamed), standing in for it.
    Edited,
    User,
}

/// A preset as the library offers it.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub sound: Sound,
    pub favorite: bool,
    pub origin: Origin,
    /// The factory preset it stands in for.
    pub replaces: Option<String>,
    file: Option<PathBuf>,
}

fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// A sound from its table (`name`, `description?`, `tags?`, `values`).
fn sound_of(t: &toml::Table) -> Result<Sound, String> {
    let name = t
        .get("name")
        .and_then(Value::as_str)
        .ok_or("a preset has no `name`")?
        .to_owned();
    let description = t
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let tags = match t.get("tags") {
        None => Vec::new(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|v| v.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| format!("{name}: `tags` holds text only"))?,
        Some(_) => return Err(format!("{name}: `tags` is a list")),
    };
    let mut values = Vec::new();
    if let Some(v) = t.get("values") {
        let v = v
            .as_table()
            .ok_or_else(|| format!("{name}: `values` is a table"))?;
        for (k, x) in v {
            let x = match x {
                Value::Integer(i) => *i as f64,
                Value::Float(f) => *f,
                Value::Boolean(b) => f64::from(u8::from(*b)),
                _ => return Err(format!("{name}: `{k}` is not a number")),
            };
            values.push((k.clone(), x));
        }
    }
    Ok(Sound {
        name,
        description,
        tags,
        values,
    })
}

/// The factory presets' file, built in whole.
const FACTORY_FILE: &str = include_str!("../sounds/presets.toml");

/// The factory's presets.
pub fn factory() -> &'static [Sound] {
    static FACTORY: OnceLock<Vec<Sound>> = OnceLock::new();
    FACTORY.get_or_init(|| parse_factory(FACTORY_FILE).unwrap_or_default())
}

/// A factory file's presets.
pub fn parse_factory(text: &str) -> Result<Vec<Sound>, String> {
    let t: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    match t.get("format").and_then(Value::as_integer) {
        Some(FORMAT) => {}
        other => return Err(format!("presets of format {other:?}, not {FORMAT}")),
    }
    match t.get("preset") {
        None => Ok(Vec::new()),
        Some(Value::Array(a)) => a
            .iter()
            .map(|p| {
                p.as_table()
                    .ok_or_else(|| "a preset is a table".to_owned())
                    .and_then(sound_of)
            })
            .collect(),
        Some(_) => Err("`preset` is a list".into()),
    }
}

/// Whether a file named `stem` (before its first dot) is one of Windows's devices.
fn device(stem: &str) -> bool {
    let stem = stem.trim_end().to_uppercase();
    DEVICES.contains(&stem.as_str())
        || NUMBERED_DEVICES.iter().any(|d| {
            stem.strip_prefix(d).is_some_and(|n| {
                let mut c = n.chars();
                c.next()
                    .is_some_and(|c| c.is_ascii_digit() || "¹²³".contains(c))
                    && c.next().is_none()
            })
        })
}

/// Whether `name` can be a preset's (and its file's) name, on every system (decisions.md R18).
pub fn check_name(name: &str) -> Result<(), String> {
    let ok = !name.is_empty()
        && name.chars().count() <= 64
        && !name.starts_with(['.', ' '])
        && !name.ends_with([' ', '.'])
        && !name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        && !name.chars().any(char::is_control);
    if !ok {
        return Err("A name is 1 to 64 characters, without / \\ : * ? \" < > |".into());
    }
    if same(name, OVERLAY) {
        return Err(format!("{name} is the library's own file"));
    }
    if device(name.split('.').next().unwrap_or(name)) {
        return Err(format!("{name} is a name Windows keeps for a device"));
    }
    Ok(())
}

/// Tags as typed (`bass, dark`): trimmed, without repeats (ignoring case) or empty ones.
pub fn parse_tags(text: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for t in text.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        if t.chars().count() > LONGEST_TAG || t.chars().any(char::is_control) {
            return Err(format!("A tag is 1 to {LONGEST_TAG} characters: {t}"));
        }
        if !out.iter().any(|o| same(o, t)) {
            out.push(t.to_owned());
        }
    }
    if out.len() > MOST_TAGS {
        return Err(format!("At most {MOST_TAGS} tags"));
    }
    Ok(out)
}

/// The folder of the libraries the plug-in shares with the DAW.
pub fn shared_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("IDLE_FOUNDRY_SHARED") {
        return PathBuf::from(d);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(windows) {
        if let Some(a) = std::env::var_os("APPDATA") {
            return PathBuf::from(a).join("Idle Foundry");
        }
    } else if cfg!(target_os = "macos") {
        if let Some(h) = &home {
            return h.join("Library/Application Support/Idle Foundry");
        }
    } else if let Some(x) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(x).join("Idle Foundry");
    } else if let Some(h) = &home {
        return h.join(".local/share/Idle Foundry");
    }
    PathBuf::from("idle-foundry-shared")
}

#[derive(Clone, Debug, Default, PartialEq)]
struct FactoryState {
    tags: Option<Vec<String>>,
    favorite: bool,
    hidden: bool,
}

fn strings(list: &[String]) -> Value {
    Value::Array(list.iter().map(|s| Value::String(s.clone())).collect())
}

/// A factory preset's marks in the overlay changed: its table, found by name ignoring case
/// (the key kept as written), a new one if there is none.
fn change_marks(presets: &mut toml::Table, name: &str, change: impl FnOnce(&mut toml::Table)) {
    let key = presets
        .keys()
        .find(|k| *k == name)
        .or_else(|| presets.keys().find(|k| same(k, name)))
        .cloned()
        .unwrap_or_else(|| name.to_owned());
    let mut marks = match presets.remove(&key) {
        Some(Value::Table(m)) => m,
        _ => toml::Table::new(),
    };
    change(&mut marks);
    presets.insert(key, Value::Table(marks));
}

/// A switch among a preset's marks: written when on, left out when off.
fn set_mark(marks: &mut toml::Table, key: &str, on: bool) {
    if on {
        marks.insert(key.into(), Value::Boolean(true));
    } else {
        marks.remove(key);
    }
}

/// The folder's files as found: each one's name, size and modification time.
type Seen = Vec<(OsString, u64, Option<SystemTime>)>;

/// The presets as read, and the files that could not be.
type Read = (Vec<Entry>, Vec<String>);

/// The library: its folder.
#[derive(Clone, Debug)]
pub struct Library {
    dir: PathBuf,
    /// The presets as last read and the folder as it was then: read again only when a file's
    /// name, size or modification time changes, or this library writes (decisions.md R18).
    last: Arc<Mutex<Option<(Seen, Read)>>>,
}

impl Library {
    /// The library shared with the DAW: `<shared>/CA-72/Presets`.
    pub fn shared() -> Self {
        Library::at(shared_dir().join("CA-72").join("Presets"))
    }

    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Library {
            dir: dir.into(),
            last: Arc::default(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn overlay_file(&self) -> PathBuf {
        self.dir.join(format!("{OVERLAY}.toml"))
    }

    /// The overlay as written, every key kept (another program's, a newer version's), so
    /// writing it again loses none: empty when there is none; an error when it cannot be
    /// read, and it is then never written over (decisions.md R18).
    fn overlay_table(&self) -> Result<toml::Table, String> {
        let file = self.overlay_file();
        let unreadable = |why: &str| {
            let why = why.lines().next().unwrap_or_default();
            format!("{} cannot be read ({why}): left as it is", file.display())
        };
        let text = match std::fs::read_to_string(&file) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(toml::Table::new()),
            Err(e) => return Err(unreadable(&e.to_string())),
        };
        let t: toml::Table = text
            .parse()
            .map_err(|e: toml::de::Error| unreadable(&e.to_string()))?;
        if !matches!(t.get("format"), None | Some(Value::Integer(FORMAT))) {
            return Err(unreadable("a format this does not know"));
        }
        if !matches!(t.get("presets"), None | Some(Value::Table(_))) {
            return Err(unreadable("`presets` is not a table"));
        }
        Ok(t)
    }

    /// The factory presets' marks, by name as written.
    fn read_overlay(&self) -> Result<BTreeMap<String, FactoryState>, String> {
        let t = self.overlay_table()?;
        let Some(Value::Table(p)) = t.get("presets") else {
            return Ok(BTreeMap::new());
        };
        Ok(p.iter()
            .filter_map(|(name, s)| {
                let s = s.as_table()?;
                Some((
                    name.clone(),
                    FactoryState {
                        tags: s.get("tags").and_then(Value::as_array).map(|a| {
                            a.iter()
                                .filter_map(|t| t.as_str().map(str::to_owned))
                                .collect()
                        }),
                        favorite: s.get("favorite").and_then(Value::as_bool).unwrap_or(false),
                        hidden: s.get("hidden").and_then(Value::as_bool).unwrap_or(false),
                    },
                ))
            })
            .collect())
    }

    /// The overlay's presets changed by `change` and the file written again, every other key
    /// as it was; not written when nothing changed, nor when it cannot be read.
    fn change_overlay(&self, change: impl FnOnce(&mut toml::Table)) -> Result<(), String> {
        let mut t = self.overlay_table()?;
        let mut presets = match t.remove("presets") {
            Some(Value::Table(p)) => p,
            _ => toml::Table::new(),
        };
        let before = presets.clone();
        change(&mut presets);
        // A preset left with no mark.
        presets.retain(|_, v| v.as_table().is_none_or(|m| !m.is_empty()));
        if presets == before {
            return Ok(());
        }
        t.entry("format").or_insert(Value::Integer(FORMAT));
        t.insert("presets".into(), Value::Table(presets));
        self.put(&self.overlay_file(), &t)
    }

    fn user_file(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.toml"))
    }

    /// The user's presets (with their files, favourites and what they stand in for), and
    /// the files that could not be read.
    #[allow(clippy::type_complexity)]
    fn users(&self) -> (Vec<(PathBuf, Sound, bool, Option<String>)>, Vec<String>) {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&self.dir)
            .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default();
        files.sort();
        let (mut out, mut bad) = (Vec::new(), Vec::new());
        for f in files {
            if f.extension().is_none_or(|e| e != "toml")
                || f.file_stem()
                    .is_some_and(|n| same(&n.to_string_lossy(), OVERLAY))
            {
                continue;
            }
            let read = std::fs::read_to_string(&f)
                .map_err(|e| e.to_string())
                .and_then(|s| s.parse::<toml::Table>().map_err(|e| e.to_string()))
                .and_then(|t| {
                    match t.get("format").and_then(Value::as_integer) {
                        Some(FORMAT) | None => {}
                        Some(other) => return Err(format!("format {other}")),
                    }
                    let sound = sound_of(&t)?;
                    // A name another program wrote is checked as one typed here.
                    check_name(&sound.name).map_err(|e| format!("{:?}: {e}", sound.name))?;
                    let fav = t.get("favorite").and_then(Value::as_bool).unwrap_or(false);
                    let rep = t.get("replaces").and_then(Value::as_str).map(str::to_owned);
                    Ok((sound, fav, rep))
                });
            match read {
                Ok((s, fav, rep)) => out.push((f, s, fav, rep)),
                Err(e) => bad.push(format!("{}: {e}", f.display())),
            }
        }
        (out, bad)
    }

    /// The folder's files as they are now.
    fn seen(&self) -> Seen {
        let mut seen: Seen = std::fs::read_dir(&self.dir)
            .map(|rd| {
                rd.filter_map(Result::ok)
                    .map(|e| {
                        let m = std::fs::metadata(e.path()).ok();
                        let size = m.as_ref().map_or(0, std::fs::Metadata::len);
                        (e.file_name(), size, m.and_then(|m| m.modified().ok()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        seen.sort();
        seen
    }

    /// The presets as read forgotten: this library changed the folder (a file rewritten
    /// within the file system's time resolution, its size the same, would not show).
    fn forget(&self) {
        if let Ok(mut last) = self.last.lock() {
            *last = None;
        }
    }

    /// A table written to the folder.
    fn put(&self, file: &Path, t: &toml::Table) -> Result<(), String> {
        let r = write(file, t);
        self.forget();
        r
    }

    /// A file of the folder deleted.
    fn delete(&self, file: &Path) -> Result<(), String> {
        let r = std::fs::remove_file(file).map_err(|e| format!("{}: {e}", file.display()));
        self.forget();
        r
    }

    /// Every preset, by name (ignoring case), and the files that could not be read: as last
    /// read while the folder is as it was then (decisions.md R18).
    pub fn entries(&self) -> (Vec<Entry>, Vec<String>) {
        let seen = self.seen();
        if let Ok(last) = self.last.lock()
            && let Some((then, read)) = last.as_ref()
            && *then == seen
        {
            return read.clone();
        }
        let read = self.read();
        if let Ok(mut last) = self.last.lock() {
            *last = Some((seen, read.clone()));
        }
        read
    }

    /// Every preset, read from the folder.
    fn read(&self) -> Read {
        let factory = factory();
        let (overlay, mut bad) = match self.read_overlay() {
            Ok(o) => (o, Vec::new()),
            Err(e) => (BTreeMap::new(), vec![e]),
        };
        let marks = |name: &str| {
            overlay
                .get(name)
                .or_else(|| overlay.iter().find(|(k, _)| same(k, name)).map(|(_, s)| s))
                .cloned()
                .unwrap_or_default()
        };
        let (mut users, unread) = self.users();
        bad.extend(unread);
        // A name is one preset's, ignoring case, on every file system (decisions.md R18): a
        // second file of it is reported, the first by file name listed.
        let mut names: Vec<String> = Vec::new();
        users.retain(|(f, s, _, _)| {
            let again = names.iter().any(|n| same(n, &s.name));
            if again {
                bad.push(format!("{}: a second preset {}", f.display(), s.name));
            } else {
                names.push(s.name.clone());
            }
            !again
        });
        // The factory preset a user's stands in for, spelt as the factory spells it: the one
        // it `replaces`, else the one of its name.
        let stands_for = |s: &Sound, r: &Option<String>| -> Option<String> {
            [r.as_deref(), Some(s.name.as_str())]
                .into_iter()
                .flatten()
                .find_map(|n| factory.iter().find(|f| same(&f.name, n)))
                .map(|f| f.name.clone())
        };
        let replaced: Vec<String> = users
            .iter()
            .filter_map(|(_, s, _, r)| stands_for(s, r))
            .collect();
        let mut list: Vec<Entry> = factory
            .iter()
            .filter(|f| !marks(&f.name).hidden && !replaced.iter().any(|r| same(r, &f.name)))
            .map(|f| {
                let state = marks(&f.name);
                let mut sound = f.clone();
                if let Some(t) = state.tags {
                    sound.tags = t;
                }
                Entry {
                    sound,
                    favorite: state.favorite,
                    origin: Origin::Factory,
                    replaces: None,
                    file: None,
                }
            })
            .collect();
        for (file, sound, favorite, r) in users {
            let replaces = stands_for(&sound, &r);
            list.push(Entry {
                origin: if replaces.is_some() {
                    Origin::Edited
                } else {
                    Origin::User
                },
                sound,
                favorite,
                replaces,
                file: Some(file),
            });
        }
        list.sort_by(|a, b| {
            a.sound
                .name
                .to_lowercase()
                .cmp(&b.sound.name.to_lowercase())
                .then(a.sound.name.cmp(&b.sound.name))
        });
        (list, bad)
    }

    /// The preset of this name (ignoring case).
    pub fn find(&self, name: &str) -> Option<Entry> {
        self.entries()
            .0
            .into_iter()
            .find(|e| same(&e.sound.name, name))
    }

    /// The presets that match ([`matching`]).
    pub fn search(&self, query: &str, tags: &[String], favorite: bool, mine: bool) -> Vec<Entry> {
        matching(&self.entries().0, query, tags, favorite, mine)
    }

    /// Every tag in use, by name ([`tags_of`]).
    pub fn tags(&self) -> Vec<String> {
        tags_of(&self.entries().0)
    }

    /// Whether `file` is free for a preset: no file of the folder has its name, ignoring case
    /// (where the file system ignores it, that is the same file), but `own`.
    fn free(&self, file: &Path, own: Option<&Path>) -> bool {
        let Some(want) = file.file_name().map(|n| n.to_string_lossy()) else {
            return false;
        };
        let own = own.and_then(Path::file_name);
        std::fs::read_dir(&self.dir).map_or(true, |rd| {
            rd.filter_map(Result::ok).all(|e| {
                let n = e.file_name();
                Some(n.as_os_str()) == own || !same(&n.to_string_lossy(), &want)
            })
        })
    }

    /// The file a preset named `name` is written to (decisions.md R18): `<name>.toml`, its own
    /// file (`own`) moved there first when that is free, so a new name (if only in case) leaves
    /// no second file on any file system; else its own file where it is. Never another's.
    fn place(&self, name: &str, own: Option<&Path>) -> Result<PathBuf, String> {
        let file = self.user_file(name);
        match own {
            Some(own) if own == file => Ok(file),
            Some(own) if self.free(&file, Some(own)) => {
                let moved = std::fs::rename(own, &file);
                self.forget();
                match moved {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                        Err(format!("{}: {e}", file.display()))
                    }
                    _ => Ok(file),
                }
            }
            Some(own) => Ok(own.to_path_buf()),
            None if self.free(&file, None) => Ok(file),
            None => Err(format!("There is already a file {}", file.display())),
        }
    }

    /// A preset written whole to `file`.
    fn write_user(
        &self,
        file: &Path,
        sound: &Sound,
        favorite: bool,
        replaces: Option<&str>,
    ) -> Result<(), String> {
        let mut t = toml::Table::new();
        t.insert("format".into(), Value::Integer(FORMAT));
        t.insert("name".into(), Value::String(sound.name.clone()));
        if !sound.description.is_empty() {
            t.insert(
                "description".into(),
                Value::String(sound.description.clone()),
            );
        }
        t.insert("tags".into(), strings(&sound.tags));
        t.insert("favorite".into(), Value::Boolean(favorite));
        if let Some(r) = replaces {
            t.insert("replaces".into(), Value::String(r.to_owned()));
        }
        let mut values = toml::Table::new();
        for (k, x) in &sound.values {
            let v = if x.fract() == 0.0 && x.abs() < 1e6 {
                Value::Integer(*x as i64)
            } else {
                Value::Float(*x)
            };
            values.insert(k.clone(), v);
        }
        t.insert("values".into(), Value::Table(values));
        self.put(file, &t)
    }

    /// The current sound saved as `name` (`values` plain, by parameter ID): replacing the
    /// user's preset of that name (its description, tags and favourite kept unless `tags`
    /// is given), or, named as a factory preset, the user's version standing in for it.
    pub fn save(
        &self,
        name: &str,
        values: Vec<(String, f64)>,
        tags: Option<Vec<String>>,
    ) -> Result<Entry, String> {
        let name = name.trim();
        check_name(name)?;
        let existing = self.find(name);
        let factory = factory()
            .iter()
            .find(|f| same(&f.name, name))
            .map(|f| f.name.clone());
        let (description, old_tags, favorite, replaces, own) = match existing {
            Some(e) => (
                e.sound.description,
                e.sound.tags,
                e.favorite,
                e.replaces.or(factory),
                e.file,
            ),
            None => (String::new(), Vec::new(), false, factory, None),
        };
        let sound = Sound {
            name: name.to_owned(),
            description,
            tags: tags.unwrap_or(old_tags),
            values,
        };
        let file = self.place(name, own.as_deref())?;
        self.write_user(&file, &sound, favorite, replaces.as_deref())?;
        self.find(name)
            .ok_or_else(|| format!("{name} was not saved"))
    }

    /// A preset renamed; a factory preset renamed is the user's version standing in for it.
    pub fn rename(&self, name: &str, to: &str) -> Result<Entry, String> {
        let to = to.trim();
        check_name(to)?;
        let e = self.find(name).ok_or_else(|| format!("No preset {name}"))?;
        if self
            .entries()
            .0
            .iter()
            .any(|o| same(&o.sound.name, to) && !same(&o.sound.name, &e.sound.name))
        {
            return Err(format!("There is already a preset {to}"));
        }
        let replaces = e
            .replaces
            .clone()
            .or_else(|| (e.origin == Origin::Factory).then(|| e.sound.name.clone()));
        let mut sound = e.sound.clone();
        sound.name = to.to_owned();
        let file = self.place(to, e.file.as_deref())?;
        self.write_user(&file, &sound, e.favorite, replaces.as_deref())?;
        self.find(to).ok_or_else(|| format!("{to} was not saved"))
    }

    fn mark(
        &self,
        e: &Entry,
        tags: Option<Vec<String>>,
        favorite: Option<bool>,
    ) -> Result<(), String> {
        match &e.file {
            // Its own file, whatever its name says (decisions.md R18).
            Some(file) => {
                let mut sound = e.sound.clone();
                if let Some(t) = tags {
                    sound.tags = t;
                }
                self.write_user(
                    file,
                    &sound,
                    favorite.unwrap_or(e.favorite),
                    e.replaces.as_deref(),
                )
            }
            None => self.change_overlay(|p| {
                change_marks(p, &e.sound.name, |m| {
                    if let Some(t) = tags {
                        m.insert("tags".into(), strings(&t));
                    }
                    if let Some(f) = favorite {
                        set_mark(m, "favorite", f);
                    }
                })
            }),
        }
    }

    /// A preset's tags, replaced.
    pub fn tag(&self, name: &str, tags: Vec<String>) -> Result<(), String> {
        let e = self.find(name).ok_or_else(|| format!("No preset {name}"))?;
        self.mark(&e, Some(tags), None)
    }

    /// A preset made a favourite, or not.
    pub fn favorite(&self, name: &str, on: bool) -> Result<(), String> {
        let e = self.find(name).ok_or_else(|| format!("No preset {name}"))?;
        self.mark(&e, None, Some(on))
    }

    /// A preset deleted: the user's file; a factory preset (or the one the user's version
    /// stood in for) hidden until [`Library::restore`].
    pub fn remove(&self, name: &str) -> Result<(), String> {
        let e = self.find(name).ok_or_else(|| format!("No preset {name}"))?;
        let hide = match e.origin {
            Origin::Factory => Some(e.sound.name.clone()),
            Origin::Edited => e.replaces.clone(),
            Origin::User => None,
        };
        // Hidden first: an overlay that cannot be written leaves the user's file.
        if let Some(h) = hide {
            self.change_overlay(|p| change_marks(p, &h, |m| set_mark(m, "hidden", true)))?;
        }
        if let Some(f) = &e.file {
            self.delete(f)?;
        }
        Ok(())
    }

    /// The user's version of a factory preset taken back: the factory's shown again (its
    /// name).
    pub fn revert(&self, name: &str) -> Result<String, String> {
        let e = self.find(name).ok_or_else(|| format!("No preset {name}"))?;
        let (Origin::Edited, Some(file), Some(factory)) = (e.origin, &e.file, &e.replaces) else {
            return Err(format!("{name} is not a factory preset edited"));
        };
        self.change_overlay(|p| change_marks(p, factory, |m| set_mark(m, "hidden", false)))?;
        self.delete(file)?;
        Ok(factory.clone())
    }

    /// The factory presets deleted shown again: their names.
    pub fn restore(&self) -> Result<Vec<String>, String> {
        let mut restored = Vec::new();
        self.change_overlay(|p| {
            for (name, m) in p.iter_mut() {
                if let Value::Table(m) = m
                    && m.get("hidden").and_then(Value::as_bool) == Some(true)
                {
                    m.remove("hidden");
                    restored.push(name.clone());
                }
            }
        })?;
        Ok(restored)
    }
}

/// The presets of `list` that match: each of `query`'s words in the name, the description or
/// a tag (ignoring case); every one of `tags`; favourites only; the user's only (and the
/// factory's edited).
pub fn matching(
    list: &[Entry],
    query: &str,
    tags: &[String],
    favorite: bool,
    mine: bool,
) -> Vec<Entry> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    list.iter()
        .filter(|e| {
            let text = format!(
                "{} {} {}",
                e.sound.name,
                e.sound.description,
                e.sound.tags.join(" ")
            )
            .to_lowercase();
            words.iter().all(|w| text.contains(w.as_str()))
                && tags.iter().all(|t| e.sound.tags.iter().any(|x| same(x, t)))
                && (!favorite || e.favorite)
                && (!mine || e.origin != Origin::Factory)
        })
        .cloned()
        .collect()
}

/// Every tag `list` uses, by name.
pub fn tags_of(list: &[Entry]) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    for e in list {
        for t in &e.sound.tags {
            if !all.iter().any(|a| same(a, t)) {
                all.push(t.clone());
            }
        }
    }
    all.sort_by_key(|t| t.to_lowercase());
    all
}

/// A table written whole, then moved into place.
fn write(file: &Path, t: &toml::Table) -> Result<(), String> {
    let text = toml::to_string(t).map_err(|e| e.to_string())?;
    if let Some(d) = file.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let tmp = file.with_extension("toml.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, file).map_err(|e| format!("{}: {e}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[Entry]) -> Vec<&str> {
        list.iter().map(|e| e.sound.name.as_str()).collect()
    }

    #[test]
    fn the_factory_file_is_read_whole() {
        let f = factory();
        assert_eq!(f.len(), 24);
        assert_eq!(f[0].name, "Bass");
        assert!(f.iter().all(|s| !s.values.is_empty()));
        assert!(
            f.iter()
                .all(|s| s.values.iter().all(|(k, _)| !KEPT.contains(&k.as_str())))
        );
    }

    /// The factory file ships inside the plug-in whole, its comments too: no trademark, no
    /// other product's or artist's name, no private record (decisions.md R1).
    #[test]
    fn the_factory_file_names_no_trademark() {
        // Its words: the comments, the names, descriptions and tags (not the values).
        let text = FACTORY_FILE
            .lines()
            .filter(|l| l.starts_with('#') || l.contains('"'))
            .collect::<Vec<_>>()
            .join("\n")
            .to_lowercase();
        for word in ["moog", "model d", "808", "909", "303", "roland", "minimoog"] {
            assert!(!text.contains(word), "the factory file says {word:?}");
        }
    }

    #[test]
    fn the_users_presets_are_saved_found_tagged_renamed_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::at(dir.path());
        let values = vec![("cutoff".to_owned(), -2.5), ("osc1_range".to_owned(), 3.0)];
        let e = lib
            .save(
                "Deep Bass",
                values,
                Some(parse_tags("bass, midnight, Bass").unwrap()),
            )
            .unwrap();
        assert_eq!(e.origin, Origin::User);
        assert_eq!(e.sound.tags, ["bass", "midnight"]);
        // The file as the DAW reads it.
        let t: toml::Table = std::fs::read_to_string(dir.path().join("Deep Bass.toml"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(t["format"].as_integer(), Some(1));
        assert_eq!(t["values"]["cutoff"].as_float(), Some(-2.5));
        assert_eq!(t["values"]["osc1_range"].as_integer(), Some(3));
        assert_eq!(
            names(&lib.search("ROUND deep", &[], false, false)),
            Vec::<&str>::new()
        );
        assert_eq!(names(&lib.search("deep", &[], false, false)), ["Deep Bass"]);
        assert_eq!(
            names(&lib.search("", &["midnight".into()], false, false)),
            ["Deep Bass"]
        );
        assert_eq!(names(&lib.search("", &[], false, true)), ["Deep Bass"]);
        lib.favorite("deep bass", true).unwrap();
        lib.favorite("Lead", true).unwrap();
        assert_eq!(
            names(&lib.search("", &[], true, false)),
            ["Deep Bass", "Lead"]
        );
        lib.tag("Bass", vec!["bass".into(), "mine".into()]).unwrap();
        assert_eq!(
            names(&lib.search("", &["mine".into()], false, false)),
            ["Bass"]
        );
        assert!(lib.tags().contains(&"midnight".to_owned()));
        lib.rename("Deep Bass", "Deeper Bass").unwrap();
        assert!(!dir.path().join("Deep Bass.toml").exists());
        assert!(lib.find("Deeper Bass").unwrap().favorite);
        assert!(lib.rename("Deeper Bass", "lead").is_err());
        lib.remove("Deeper Bass").unwrap();
        assert!(lib.search("", &[], false, true).is_empty());
        assert!(check_name("a/b").is_err() && check_name(" x").is_err() && check_name("").is_err());
    }

    #[test]
    fn a_factory_preset_edited_renamed_or_removed_comes_back() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::at(dir.path());
        let edited = lib
            .save("lead", vec![("emphasis".into(), 9.0)], None)
            .unwrap();
        assert_eq!(edited.origin, Origin::Edited);
        assert_eq!(edited.replaces.as_deref(), Some("Lead"));
        assert_eq!(edited.sound.tags, ["lead", "glide"]);
        assert_eq!(
            lib.entries()
                .0
                .iter()
                .filter(|e| same(&e.sound.name, "lead"))
                .count(),
            1
        );
        assert_eq!(lib.revert("lead").unwrap(), "Lead");
        assert_eq!(lib.find("Lead").unwrap().origin, Origin::Factory);
        lib.rename("Bass", "Round Bass").unwrap();
        assert!(lib.find("Bass").is_none());
        assert_eq!(lib.find("Round Bass").unwrap().origin, Origin::Edited);
        lib.revert("Round Bass").unwrap();
        assert!(lib.find("Bass").is_some());
        lib.remove("Bass").unwrap();
        lib.save("Lead", vec![], None).unwrap();
        lib.remove("Lead").unwrap();
        assert!(lib.find("Bass").is_none() && lib.find("Lead").is_none());
        assert!(!dir.path().join("Lead.toml").exists());
        assert_eq!(lib.restore().unwrap(), ["Bass", "Lead"]);
        assert_eq!(lib.entries().0.len(), factory().len());
        assert!(lib.revert("Bass").is_err());
    }

    /// `factory` is the folder's own file (the factory presets' marks): no preset takes the
    /// name, in any case, and the marks survive the try (decisions.md R18).
    #[test]
    fn factory_is_no_presets_name() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::at(dir.path());
        lib.favorite("Lead", true).unwrap();
        lib.tag("Bass", vec!["mine".into()]).unwrap();
        for n in ["factory", "Factory", "FACTORY"] {
            assert!(
                lib.save(n, vec![("cutoff".into(), 1.0)], None).is_err(),
                "{n}"
            );
            assert!(lib.rename("Lead", n).is_err(), "{n}");
        }
        assert!(lib.find("Lead").unwrap().favorite);
        assert_eq!(lib.find("Bass").unwrap().sound.tags, ["mine"]);
        assert!(lib.save("Factory Floor", vec![], None).is_ok());
    }

    /// Windows's device names (an extension or not, any case) are no preset's on any system,
    /// nor are names starting or ending with a space or a dot (decisions.md R18).
    #[test]
    fn windows_device_names_are_refused_everywhere() {
        for n in [
            "CON",
            "con",
            "Prn",
            "AUX",
            "NUL",
            "nul.txt",
            "NUL.tar.gz",
            "COM1",
            "com9",
            "COM0",
            "LPT1",
            "lpt9.x",
            "COM¹",
            "LPT³",
            "CONIN$",
            "conout$",
            "Aux .x",
            "Dot.",
            "Space ",
            " Space",
            ".Dot",
        ] {
            assert!(check_name(n).is_err(), "{n:?}");
        }
        for n in [
            "CONSOLE",
            "Nullify",
            "COM10",
            "LPT",
            "Com Bass",
            "Auxiliary Lead",
            "a.con",
            "Bass 2",
        ] {
            assert!(check_name(n).is_ok(), "{n:?}");
        }
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::at(dir.path());
        assert!(lib.save("aux", vec![], None).is_err());
        assert!(lib.rename("Lead", "NUL.lead").is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    /// A file's `name` is checked as a typed one: a name reaching out of the folder, the
    /// folder's own file's or a device's is reported, not listed (decisions.md R18).
    #[test]
    fn a_files_unsafe_name_is_reported_not_listed() {
        let dir = tempfile::tempdir().unwrap();
        for (file, name) in [
            ("up.toml", "../../outside"),
            ("root.toml", "/tmp/outside"),
            ("device.toml", "NUL"),
            ("own.toml", "factory"),
            ("fine.toml", "Fine"),
        ] {
            std::fs::write(
                dir.path().join(file),
                format!("format = 1\nname = {name:?}\n\n[values]\ncutoff = 1\n"),
            )
            .unwrap();
        }
        let lib = Library::at(dir.path());
        let (list, bad) = lib.entries();
        let mine: Vec<&str> = list
            .iter()
            .filter(|e| e.origin == Origin::User)
            .map(|e| e.sound.name.as_str())
            .collect();
        assert_eq!(mine, ["Fine"]);
        assert_eq!(bad.len(), 4, "{bad:?}");
    }

    /// An overlay that cannot be read (a hand edit gone wrong, a newer format) is never
    /// written over: marking, deleting or restoring a factory preset says why and changes
    /// nothing, the user's version deleted included; the presets still list (decisions.md
    /// R18).
    #[test]
    fn an_unreadable_overlay_is_left_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::at(dir.path());
        lib.save("Lead", vec![("cutoff".into(), 1.0)], None)
            .unwrap();
        let overlay = dir.path().join("factory.toml");
        for text in [
            "format = 1\n[presets.Bass\nfavorite = true\n",
            "format = 2\n[presets.Bass]\nfavorite = true\nrating = 5\n",
            "format = 1\npresets = 3\n",
        ] {
            std::fs::write(&overlay, text).unwrap();
            let e = lib.favorite("Bass", true).unwrap_err();
            assert!(e.contains("factory.toml cannot be read"), "{e}");
            assert!(lib.tag("Bass", vec!["x".into()]).is_err());
            assert!(lib.remove("Bass").is_err());
            assert!(lib.remove("Lead").is_err());
            assert!(lib.revert("Lead").is_err());
            assert!(lib.restore().is_err());
            assert_eq!(std::fs::read_to_string(&overlay).unwrap(), text);
            assert!(dir.path().join("Lead.toml").exists());
            let (list, bad) = lib.entries();
            assert_eq!(list.len(), factory().len());
            assert_eq!(bad.len(), 1, "{bad:?}");
        }
    }

    /// What the overlay holds beyond these marks (another program's keys, a key in another
    /// case) is kept when it is written again (decisions.md R18).
    #[test]
    fn the_overlay_keeps_what_it_does_not_know() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::at(dir.path());
        let overlay = dir.path().join("factory.toml");
        std::fs::write(
            &overlay,
            "format = 1\nnote = \"kept\"\n\n[presets.Lead]\nfavorite = true\nrating = 5\n\n[presets.bass]\nhidden = true\ntags = [\"low\"]\n",
        )
        .unwrap();
        assert!(lib.find("Lead").unwrap().favorite);
        assert!(lib.find("Bass").is_none(), "hidden whatever the key's case");
        lib.favorite("Lead", false).unwrap();
        assert_eq!(lib.restore().unwrap(), ["bass"]);
        assert_eq!(lib.find("Bass").unwrap().sound.tags, ["low"]);
        lib.tag("Bass", vec!["round".into()]).unwrap();
        let t: toml::Table = std::fs::read_to_string(&overlay).unwrap().parse().unwrap();
        assert_eq!(t["note"].as_str(), Some("kept"));
        assert_eq!(t["presets"]["Lead"]["rating"].as_integer(), Some(5));
        assert!(t["presets"]["Lead"].get("favorite").is_none());
        assert_eq!(t["presets"]["bass"]["tags"][0].as_str(), Some("round"));
        assert!(t["presets"].get("Bass").is_none());
        assert_eq!(lib.find("Bass").unwrap().sound.tags, ["round"]);
    }

    fn toml_files(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".toml"))
            .collect();
        v.sort();
        v
    }

    /// A star or tags go to the preset's own file, whatever its name says: one file, one row,
    /// and nothing written where the name alone would put it (decisions.md R18).
    #[test]
    fn marks_go_to_the_presets_own_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("my_patch.toml"),
            "format = 1\nname = \"My Patch\"\n\n[values]\ncutoff = 1\n",
        )
        .unwrap();
        let lib = Library::at(dir.path());
        lib.favorite("My Patch", true).unwrap();
        lib.tag("my patch", vec!["round".into()]).unwrap();
        assert_eq!(toml_files(dir.path()), ["my_patch.toml"]);
        let e = lib.find("My Patch").unwrap();
        assert!(e.favorite && e.sound.tags == ["round"]);
        assert_eq!(lib.search("", &[], false, true).len(), 1);
        // Saved again, it takes its name's file (nothing else there), still one.
        lib.save("My Patch", vec![("cutoff".into(), 2.0)], None)
            .unwrap();
        assert_eq!(toml_files(dir.path()), ["My Patch.toml"]);
        assert!(lib.find("My Patch").unwrap().favorite);
    }

    /// A name changed only in case leaves one file, on a file system that tells cases apart
    /// too (decisions.md R18).
    #[test]
    fn a_name_changed_in_case_leaves_one_file() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::at(dir.path());
        lib.save(
            "Deep Bass",
            vec![("cutoff".into(), 1.0)],
            Some(vec!["low".into()]),
        )
        .unwrap();
        lib.save("deep bass", vec![("cutoff".into(), 2.0)], None)
            .unwrap();
        assert_eq!(toml_files(dir.path()), ["deep bass.toml"]);
        let e = lib.find("Deep Bass").unwrap();
        assert_eq!(e.sound.name, "deep bass");
        assert_eq!(e.sound.tags, ["low"]);
        assert!(e.sound.values.contains(&("cutoff".to_owned(), 2.0)));
        lib.rename("deep bass", "DEEP BASS").unwrap();
        assert_eq!(toml_files(dir.path()), ["DEEP BASS.toml"]);
        assert_eq!(lib.search("", &[], false, true).len(), 1);
    }

    /// Two files of one name (in any case) list once, the other reported; a file in the way
    /// of a name (not the preset's) is never written over (decisions.md R18).
    #[test]
    fn a_name_is_one_presets_and_another_file_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let preset = |file: &str, name: &str| {
            std::fs::write(
                dir.path().join(file),
                format!("format = 1\nname = {name:?}\n\n[values]\ncutoff = 1\n"),
            )
            .unwrap();
        };
        preset("a.toml", "Twin");
        preset("b.toml", "twin");
        preset("c.toml", "Third");
        std::fs::write(dir.path().join("Third.toml"), "name = ").unwrap();
        std::fs::write(dir.path().join("Fourth.toml"), "format = 9\n").unwrap();
        let lib = Library::at(dir.path());
        let (list, bad) = lib.entries();
        let twins = list.iter().filter(|e| same(&e.sound.name, "twin")).count();
        assert_eq!(twins, 1);
        assert_eq!(bad.len(), 3, "{bad:?}");
        // Third's own file takes it; the file of its name is not its own and stays.
        lib.save("Third", vec![("cutoff".into(), 3.0)], None)
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("Third.toml")).unwrap(),
            "name = "
        );
        assert!(
            lib.find("Third")
                .unwrap()
                .sound
                .values
                .contains(&("cutoff".to_owned(), 3.0))
        );
        assert!(lib.save("Fourth", vec![], None).is_err());
        assert!(lib.rename("Twin", "fourth").is_ok(), "into its own file");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("Fourth.toml")).unwrap(),
            "format = 9\n"
        );
    }

    /// A user's preset that `replaces` a factory one, in another case, stands in for it as
    /// the factory spells it: deleting it hides that one (decisions.md R18).
    #[test]
    fn replaces_is_the_factorys_name_in_any_case() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Mine.toml"),
            "format = 1\nname = \"Mine\"\nreplaces = \"lead\"\n\n[values]\ncutoff = 1\n",
        )
        .unwrap();
        let lib = Library::at(dir.path());
        let e = lib.find("Mine").unwrap();
        assert_eq!(
            (e.origin, e.replaces.as_deref()),
            (Origin::Edited, Some("Lead"))
        );
        assert!(lib.find("Lead").is_none());
        lib.remove("Mine").unwrap();
        assert!(lib.find("Lead").is_none() && lib.find("Mine").is_none());
        assert_eq!(lib.restore().unwrap(), ["Lead"]);
    }

    /// The folder is parsed again only when it changed: a file of the same name, size and
    /// time is taken as read; another time, a file more, or a write of its own is read
    /// (decisions.md R18).
    #[test]
    fn the_folder_is_read_again_only_when_it_changes() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::at(dir.path());
        let file = dir.path().join("Pad.toml");
        let text = "format = 1\nname = \"Pad\"\n\n[values]\ncutoff = 1\n";
        std::fs::write(&file, text).unwrap();
        assert!(lib.find("Pad").is_some());
        let then = std::fs::metadata(&file).unwrap().modified().unwrap();
        let touch = |t: SystemTime| {
            std::fs::File::options()
                .write(true)
                .open(&file)
                .unwrap()
                .set_modified(t)
                .unwrap();
        };
        // Garbage of the same size at the same time: not parsed, Pad as read.
        std::fs::write(&file, "#".repeat(text.len())).unwrap();
        touch(then);
        assert!(
            lib.find("Pad").is_some(),
            "parsed again, the folder unchanged"
        );
        assert_eq!(lib.search("pad", &[], false, true).len(), 1);
        // Another time: parsed, and it has no name.
        touch(then + std::time::Duration::from_secs(2));
        assert!(lib.find("Pad").is_none());
        assert_eq!(lib.entries().1.len(), 1);
        // A file more, and the library's own writes.
        lib.save("Lid", vec![], Some(vec!["new".into()])).unwrap();
        assert!(lib.tags().contains(&"new".to_owned()));
        lib.favorite("Lid", true).unwrap();
        assert!(lib.find("Lid").unwrap().favorite);
    }

    /// The DAW's files (written by its own TOML writer) read here, and an unreadable one
    /// reported, not fatal.
    #[test]
    fn a_preset_written_by_the_daw_is_read() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Wide Pad.toml"),
            "format = 1\nname = \"Wide Pad\"\ntags = [\"pad\"]\nfavorite = false\n\n[values]\ncutoff = 1.5\nloudness_attack = 6\npoly = 1\nvoices = 6\nentropy = 40\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Broken.toml"), "name = ").unwrap();
        let lib = Library::at(dir.path());
        let (list, bad) = lib.entries();
        let pad = list.iter().find(|e| e.sound.name == "Wide Pad").unwrap();
        assert_eq!(pad.origin, Origin::User);
        assert!(pad.sound.values.contains(&("voices".to_owned(), 6.0)));
        assert_eq!(bad.len(), 1);
    }
}

/// Each program reads the other's files: with `CA72_CROSS_DIR` set (a library folder), writes
/// a preset there as the plug-in does and lists what is there (the DAW's included).
#[cfg(test)]
mod cross {
    use super::*;

    #[test]
    #[ignore = "a check against the DAW's files, run by hand"]
    fn cross_check() {
        let Some(dir) = std::env::var_os("CA72_CROSS_DIR") else {
            return;
        };
        let lib = Library::at(dir);
        lib.save(
            "From The Plug-in",
            vec![
                ("cutoff".into(), -1.25),
                ("osc1_range".into(), 1.0),
                ("voices".into(), 6.0),
            ],
            Some(vec!["cross".into()]),
        )
        .unwrap();
        lib.favorite("Pulse Strut", true).unwrap();
        for e in lib
            .entries()
            .0
            .iter()
            .filter(|e| e.origin != Origin::Factory || e.favorite)
        {
            eprintln!(
                "CROSS {} {:?} fav={} tags={:?} values={}",
                e.sound.name,
                e.origin,
                e.favorite,
                e.sound.tags,
                e.sound.values.len()
            );
        }
    }
}
