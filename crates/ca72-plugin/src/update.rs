//! The update check (decisions.md R27; the owner, 2026-10-06, chose a check made when the
//! user asks, and a DOWNLOAD that opens the installer in the browser). At the right of the
//! presets' drawer's tools: the plug-in's version and CHECK FOR UPDATES. A click asks GitHub's
//! API for the repository's latest release through the system's curl (the plug-in links no
//! network code of its own), which writes the answer into a file of the editor's. The editor
//! looks each frame whether curl has finished, so no thread of the plug-in waits for it (R23:
//! none may run once the host has unloaded the plug-in), and closing the editor ends curl. A
//! newer release offers DOWNLOAD, which opens its installer for this system in the browser
//! (the release's page if it has none), for the user to run with the DAW closed; a check that
//! failed offers the releases' page. Nothing is checked unasked, and nothing is sent but the
//! request.

use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use ca72_panel::presets::{Tone, UpdateScene};

/// The repository's releases, and its latest as GitHub's API gives it.
pub const RELEASES: &str = "https://github.com/idlefoundry/ca-72/releases";
const LATEST: &str = "https://api.github.com/repos/idlefoundry/ca-72/releases/latest";

/// This build's version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How long curl may take, in seconds; the editor ends it if it runs on past `PATIENCE`.
const CURL_SECONDS: u32 = 20;
const PATIENCE: Duration = Duration::from_secs(30);
/// The most the answer may be, in bytes (0.1.0's was 15 kB).
const MOST: u64 = 1 << 20;

/// What follows `CA-72-<version>-` in the name of the installer for this system, as the
/// README lists them (none where no installer is made).
const INSTALLER: Option<&str> = if cfg!(target_os = "windows") {
    Some("Windows-setup.exe")
} else if cfg!(target_os = "macos") {
    Some("macOS.pkg")
} else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    Some("Linux-x86_64.tar.gz")
} else {
    None
};

/// A version's three numbers: `0.2.0`'s (none for anything else, a pre-release among it).
fn numbers(version: &str) -> Option<(u64, u64, u64)> {
    let mut n = version.split('.').map(|p| {
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        p.parse().ok()
    });
    let v = (n.next()??, n.next()??, n.next()??);
    n.next().is_none().then_some(v)
}

/// Whether `version` is later than this build's.
fn newer(version: &str) -> bool {
    numbers(version) > numbers(VERSION)
}

/// The latest release, as the drawer offers it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// Its version: its tag without the `v`.
    pub version: String,
    /// What DOWNLOAD opens: its installer for this system, else its page.
    pub download: String,
}

/// The release in the API's answer, its installer `CA-72-<version>-<installer>`.
fn release(answer: &[u8], installer: Option<&str>) -> Option<Release> {
    let v: serde_json::Value = serde_json::from_slice(answer).ok()?;
    let tag = v.get("tag_name")?.as_str()?;
    let version = tag.strip_prefix('v').unwrap_or(tag);
    numbers(version)?;
    let page = v
        .get("html_url")
        .and_then(serde_json::Value::as_str)
        .filter(|u| ours(u))
        .unwrap_or(RELEASES);
    let name = installer.map(|i| format!("CA-72-{version}-{i}"));
    let download = v
        .get("assets")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .find(|a| {
            name.is_some() && a.get("name").and_then(serde_json::Value::as_str) == name.as_deref()
        })
        .and_then(|a| a.get("browser_download_url")?.as_str())
        .filter(|u| ours(u))
        .unwrap_or(page);
    Some(Release {
        version: version.to_owned(),
        download: download.to_owned(),
    })
}

/// Whether `url` is the repository's releases' page or one under it: the browser is sent
/// nowhere else, whatever an answer says.
fn ours(url: &str) -> bool {
    url.strip_prefix(RELEASES).is_some_and(|rest| {
        (rest.is_empty() || rest.starts_with('/'))
            && !rest.contains("..")
            && rest
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~/".contains(&b))
    })
}

/// What starts curl writing its answer to `out` (the tests start something else).
pub(crate) type Fetch = fn(out: File) -> io::Result<Child>;
/// What opens a page in the browser (the tests open none): the process it started, if any,
/// to be reaped once it ends.
pub(crate) type Open = fn(url: &str) -> io::Result<Option<Child>>;

/// The system's curl: Windows' own (Windows 10 1803 on), not one a folder on the search path
/// might hold; macOS's; elsewhere the one on the path.
fn curl_program() -> PathBuf {
    if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        PathBuf::from(root).join("System32").join("curl.exe")
    } else if cfg!(target_os = "macos") {
        PathBuf::from("/usr/bin/curl")
    } else {
        PathBuf::from("curl")
    }
}

/// curl asking GitHub's API for the latest release, its answer to `out`: quiet, failing on
/// an HTTP error, by HTTPS only, within `CURL_SECONDS`, and on Windows without a console of
/// its own (one would flash up over the host).
fn curl(out: File) -> io::Result<Child> {
    let seconds = CURL_SECONDS.to_string();
    let most = MOST.to_string();
    let agent = format!("CA-72/{VERSION}");
    let mut c = Command::new(curl_program());
    c.args([
        "--silent",
        "--fail",
        "--location",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--max-time",
        &seconds,
        "--max-filesize",
        &most,
        "--header",
        "Accept: application/vnd.github+json",
        "--user-agent",
        &agent,
        LATEST,
    ])
    .stdin(Stdio::null())
    .stdout(out)
    .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c.spawn()
}

/// `url` opened in the browser: by Windows' shell, macOS's `open`, else `xdg-open`.
fn browse(url: &str) -> io::Result<Option<Child>> {
    #[cfg(windows)]
    {
        shell::open(url).map(|()| None)
    }
    #[cfg(not(windows))]
    {
        let program = if cfg!(target_os = "macos") {
            "/usr/bin/open"
        } else {
            "xdg-open"
        };
        Command::new(program)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(Some)
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod shell {
    use std::io;
    use std::iter::once;
    use std::ptr::{null, null_mut};

    use winapi::um::shellapi::ShellExecuteW;
    use winapi::um::winuser::SW_SHOWNORMAL;

    /// `url` opened by the shell, in the user's browser.
    pub fn open(url: &str) -> io::Result<()> {
        let wide = |s: &str| s.encode_utf16().chain(once(0)).collect::<Vec<u16>>();
        let (verb, file) = (wide("open"), wide(url));
        // SAFETY: both strings are NUL-terminated UTF-16 and outlive the call; no window,
        // parameters or directory are given (null).
        let r = unsafe {
            ShellExecuteW(
                null_mut(),
                verb.as_ptr(),
                file.as_ptr(),
                null(),
                null(),
                SW_SHOWNORMAL,
            )
        };
        // Above 32 it opened it; else an error's code.
        let code = r as isize;
        if code > 32 {
            Ok(())
        } else {
            Err(io::Error::other(format!("ShellExecuteW: {code}")))
        }
    }
}

/// curl at work, its answer going into `file`.
pub(crate) struct Check {
    curl: Child,
    pub(crate) file: PathBuf,
    began: Instant,
}

/// Where a check stands.
enum Poll {
    Waiting,
    Answered(Vec<u8>),
    Failed,
}

impl Check {
    /// `fetch` started, its answer into a file of the check's own.
    fn start(fetch: Fetch) -> io::Result<Check> {
        static COUNT: AtomicU32 = AtomicU32::new(0);
        let mut tries = 0;
        let (file, out) = loop {
            let file = std::env::temp_dir().join(format!(
                "ca72-latest-release-{}-{}.json",
                std::process::id(),
                COUNT.fetch_add(1, Ordering::Relaxed)
            ));
            // Made afresh: never a file already there, nor where a link there points.
            match File::options().write(true).create_new(true).open(&file) {
                Ok(out) => break (file, out),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists && tries < 8 => tries += 1,
                Err(e) => return Err(e),
            }
        };
        match fetch(out) {
            Ok(curl) => Ok(Check {
                curl,
                file,
                began: Instant::now(),
            }),
            Err(e) => {
                let _ = std::fs::remove_file(&file);
                Err(e)
            }
        }
    }

    fn poll(&mut self) -> Poll {
        match self.curl.try_wait() {
            Ok(None) if self.began.elapsed() < PATIENCE => Poll::Waiting,
            // Too slow (ended as the check is dropped), or failed.
            Ok(None) => Poll::Failed,
            Ok(Some(status)) if !status.success() => Poll::Failed,
            // Finished; or gone with its status unknown, in a host that reaps its children
            // itself (one that ignores SIGCHLD): what it wrote is judged by itself
            // (`release`), and curl writes nothing on an HTTP error.
            Ok(Some(_)) | Err(_) => self.answer().map_or(Poll::Failed, Poll::Answered),
        }
    }

    fn answer(&self) -> Option<Vec<u8>> {
        let mut a = Vec::new();
        File::open(&self.file)
            .ok()?
            .take(MOST + 1)
            .read_to_end(&mut a)
            .ok()?;
        (a.len() as u64 <= MOST).then_some(a)
    }
}

impl Drop for Check {
    /// curl ended if it still runs, and its file removed.
    fn drop(&mut self) {
        if matches!(self.curl.try_wait(), Ok(None)) {
            let _ = self.curl.kill();
        }
        let _ = self.curl.wait();
        let _ = std::fs::remove_file(&self.file);
    }
}

/// Where the update check stands.
enum State {
    /// Not asked yet.
    Idle,
    Checking(Check),
    /// This is the latest release (or later).
    Latest,
    Newer(Release),
    /// GitHub could not be asked, or its answer not read.
    Failed,
}

/// The update check as the editor runs it: where it stands, what the drawer shows of it and
/// what its button does. Dropped (the editor closed), it ends curl.
pub struct Update {
    state: State,
    /// Whether the browser opened the last page asked of it (none asked yet).
    opened: Option<bool>,
    /// The processes opening pages (`open`, `xdg-open`), reaped as they end, one that failed
    /// counting as no browser opened; never ended by the editor (one may be the browser
    /// itself).
    openers: Vec<Child>,
    fetch: Fetch,
    open: Open,
}

impl std::fmt::Debug for Update {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Update")
            .field("scene", &self.scene())
            .finish()
    }
}

impl Default for Update {
    fn default() -> Self {
        Update::with(curl, browse)
    }
}

impl Update {
    pub(crate) fn with(fetch: Fetch, open: Open) -> Self {
        Update {
            state: State::Idle,
            opened: None,
            openers: Vec::new(),
            fetch,
            open,
        }
    }

    /// A press on its button: the check begun, the release's download or the releases'
    /// page opened.
    pub fn press(&mut self) {
        match &self.state {
            State::Idle | State::Latest => {
                self.opened = None;
                self.state = match Check::start(self.fetch) {
                    Ok(c) => State::Checking(c),
                    Err(_) => State::Failed,
                };
            }
            State::Checking(_) => {}
            State::Newer(r) => {
                let url = r.download.clone();
                self.browse(&url);
            }
            State::Failed => self.browse(RELEASES),
        }
    }

    fn browse(&mut self, url: &str) {
        match (self.open)(url) {
            Ok(opener) => {
                self.openers.extend(opener);
                self.opened = Some(true);
            }
            Err(_) => self.opened = Some(false),
        }
    }

    /// Once a frame: curl's answer read once it has finished, the openers that ended reaped.
    pub fn tick(&mut self) {
        let mut failed = false;
        self.openers.retain_mut(|o| match o.try_wait() {
            Ok(None) => true,
            Ok(Some(status)) => {
                failed |= !status.success();
                false
            }
            Err(_) => false,
        });
        if failed {
            self.opened = Some(false);
        }
        let State::Checking(c) = &mut self.state else {
            return;
        };
        self.state = match c.poll() {
            Poll::Waiting => return,
            Poll::Answered(a) => match release(&a, INSTALLER) {
                Some(r) if newer(&r.version) => State::Newer(r),
                Some(_) => State::Latest,
                None => State::Failed,
            },
            Poll::Failed => State::Failed,
        };
    }

    /// What the drawer shows of it.
    pub fn scene(&self) -> UpdateScene {
        let (text, tone, button) = match &self.state {
            State::Idle => (format!("CA-72 {VERSION}"), Tone::Dim, "CHECK FOR UPDATES"),
            State::Checking(_) => (format!("CA-72 {VERSION}"), Tone::Dim, "CHECKING…"),
            State::Latest => (
                format!("{VERSION} IS UP TO DATE"),
                Tone::Dim,
                "CHECK FOR UPDATES",
            ),
            State::Newer(_) if self.opened == Some(true) => (
                "CLOSE THE DAW, THEN INSTALL IT".to_owned(),
                Tone::News,
                "DOWNLOAD",
            ),
            State::Newer(r) => (
                format!("{} IS OUT (THIS IS {VERSION})", r.version),
                Tone::News,
                "DOWNLOAD",
            ),
            State::Failed => ("COULD NOT CHECK".to_owned(), Tone::Trouble, "RELEASES PAGE"),
        };
        let (text, tone) = if self.opened == Some(false) {
            ("NO BROWSER WOULD OPEN".to_owned(), Tone::Trouble)
        } else {
            (text, tone)
        };
        UpdateScene {
            text,
            tone,
            button: button.to_owned(),
        }
    }
}

/// Stand-ins for curl and the browser, for the editor's tests and these.
#[cfg(test)]
pub(crate) mod fake {
    use std::cell::RefCell;

    use super::*;

    thread_local! {
        static OPENED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    /// No curl to start.
    pub(crate) fn no_curl(_out: File) -> io::Result<Child> {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }

    /// A browser that opens every page (this thread's record of them: [`opened`]).
    pub(crate) fn browser(url: &str) -> io::Result<Option<Child>> {
        OPENED.with(|o| o.borrow_mut().push(url.to_owned()));
        Ok(None)
    }

    /// No browser that will open.
    pub(crate) fn no_browser(_url: &str) -> io::Result<Option<Child>> {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }

    /// The pages this thread's [`browser`] opened since it was last asked.
    pub(crate) fn opened() -> Vec<String> {
        OPENED.with(|o| std::mem::take(&mut *o.borrow_mut()))
    }
}

#[cfg(test)]
mod tests {
    use ca72_panel::fonts::Fonts;
    use ca72_panel::presets::update_fits;

    use super::fake::*;
    use super::*;

    /// GitHub's answer for a release tagged `tag`, cut to what is read, its assets those
    /// 0.1.0's had.
    fn answer(tag: &str) -> Vec<u8> {
        let v = tag.trim_start_matches('v');
        let asset = |n: &str| {
            serde_json::json!({
                "name": format!("CA-72-{v}-{n}"),
                "browser_download_url": format!("{RELEASES}/download/{tag}/CA-72-{v}-{n}"),
                "state": "uploaded",
            })
        };
        let assets: Vec<_> = [
            "git-sources.tar.gz",
            "Linux-x86_64.tar.gz",
            "macOS.pkg",
            "Windows-setup.exe",
        ]
        .into_iter()
        .map(asset)
        .collect();
        serde_json::to_vec(&serde_json::json!({
            "html_url": format!("{RELEASES}/tag/{tag}"),
            "tag_name": tag,
            "name": format!("CA-72 {v}"),
            "draft": false,
            "prerelease": false,
            "assets": assets,
        }))
        .unwrap()
    }

    #[test]
    fn a_version_is_three_numbers() {
        assert_eq!(numbers("0.1.0"), Some((0, 1, 0)));
        assert_eq!(numbers("12.0.345"), Some((12, 0, 345)));
        for odd in [
            "",
            "0.1",
            "0.1.0.1",
            "0.1.x",
            "0.2.0-beta.1",
            "v0.1.0",
            "0.+1.0",
            "0..1",
        ] {
            assert_eq!(numbers(odd), None, "{odd}");
        }
        // This build's, and the repository's addresses those the workspace's manifest names.
        assert!(numbers(VERSION).is_some());
        let manifest = include_str!("../../../Cargo.toml");
        let repository = manifest
            .lines()
            .find_map(|l| l.strip_prefix("repository = \""))
            .and_then(|l| l.strip_suffix('"'))
            .expect("the workspace's repository");
        assert_eq!(RELEASES, format!("{repository}/releases"));
        assert_eq!(
            LATEST,
            repository.replace("https://github.com/", "https://api.github.com/repos/")
                + "/releases/latest"
        );
        assert!(newer("99.0.0") && newer("0.1.1") && newer("0.2.0") && newer("1.0.0"));
        assert!(!newer(VERSION) && !newer("0.0.9") && !newer("nightly"));
    }

    #[test]
    fn the_installer_for_each_system_is_found() {
        let a = answer("v0.2.0");
        for (installer, name) in [
            ("Windows-setup.exe", "CA-72-0.2.0-Windows-setup.exe"),
            ("macOS.pkg", "CA-72-0.2.0-macOS.pkg"),
            ("Linux-x86_64.tar.gz", "CA-72-0.2.0-Linux-x86_64.tar.gz"),
        ] {
            assert_eq!(
                release(&a, Some(installer)),
                Some(Release {
                    version: "0.2.0".into(),
                    download: format!("{RELEASES}/download/v0.2.0/{name}"),
                })
            );
        }
        // None for this system, or the release without one: its page.
        let page = format!("{RELEASES}/tag/v0.2.0");
        assert_eq!(release(&a, None).unwrap().download, page);
        assert_eq!(
            release(&a, Some("Linux-aarch64.tar.gz")).unwrap().download,
            page
        );
        // This system's is one of the three.
        if let Some(installer) = INSTALLER {
            assert!(
                release(&a, INSTALLER)
                    .unwrap()
                    .download
                    .ends_with(installer)
            );
        }
    }

    /// The browser goes only to the repository's releases, whatever an answer says.
    #[test]
    fn the_browser_is_sent_only_to_the_releases() {
        let mut v: serde_json::Value = serde_json::from_slice(&answer("v0.2.0")).unwrap();
        v["assets"][3]["browser_download_url"] =
            "https://example.com/CA-72-0.2.0-Windows-setup.exe".into();
        let a = serde_json::to_vec(&v).unwrap();
        assert_eq!(
            release(&a, Some("Windows-setup.exe")).unwrap().download,
            format!("{RELEASES}/tag/v0.2.0")
        );
        v["html_url"] = "https://github.com/someone/else/releases/tag/v0.2.0".into();
        let a = serde_json::to_vec(&v).unwrap();
        assert_eq!(
            release(&a, Some("Windows-setup.exe")).unwrap().download,
            RELEASES
        );
        assert!(ours(RELEASES) && ours(&format!("{RELEASES}/tag/v0.2.0")));
        for not in [
            "https://github.com/idlefoundry/ca-72/releases.example.com/x",
            "https://github.com/idlefoundry/ca-72/releases/../../other/releases",
            "https://github.com/idlefoundry/ca-72/releases/tag/v0.2.0?x=\"y\"",
            "https://github.com/idlefoundry/ca-72/releases/tag/v 0.2.0",
            "http://github.com/idlefoundry/ca-72/releases",
            "file:///C:/Windows/System32/calc.exe",
        ] {
            assert!(!ours(not), "{not}");
        }
    }

    #[test]
    fn an_answer_without_a_version_is_none() {
        assert_eq!(release(b"", INSTALLER), None);
        assert_eq!(release(b"<html>rate limited</html>", INSTALLER), None);
        assert_eq!(release(br#"{"message":"Not Found"}"#, INSTALLER), None);
        assert_eq!(release(&answer("nightly"), INSTALLER), None);
        assert_eq!(release(&answer("v0.2"), INSTALLER), None);
        // A tag without its `v` is read too.
        assert_eq!(
            release(&answer("0.2.0"), INSTALLER).unwrap().version,
            "0.2.0"
        );
    }

    /// Each scene the check shows fits the drawer whole, a long version's among them.
    #[test]
    fn every_scene_fits_the_drawer() {
        let fonts = Fonts::new();
        let mut u = Update::with(no_curl, browser);
        let release = Release {
            version: "10.10.10".into(),
            download: RELEASES.into(),
        };
        let mut scenes = vec![u.scene()];
        u.state = State::Latest;
        scenes.push(u.scene());
        u.state = State::Newer(release);
        scenes.push(u.scene());
        u.press();
        scenes.push(u.scene());
        u.state = State::Failed;
        scenes.push(u.scene());
        u.opened = Some(false);
        scenes.push(u.scene());
        for s in &scenes {
            assert!(update_fits(&fonts, s), "{s:?}");
        }
        let checking = UpdateScene {
            button: "CHECKING…".into(),
            ..scenes[0].clone()
        };
        assert!(update_fits(&fonts, &checking));
    }

    /// A canned answer, by the release it names, for a stand-in for curl to print.
    fn canned(release: &str) -> PathBuf {
        std::env::temp_dir().join(format!("ca72-update-tests-{release}.json"))
    }

    /// Something that prints `file`, as curl prints GitHub's answer.
    fn print(file: PathBuf, out: File) -> io::Result<Child> {
        let mut c = if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.arg("/C").arg("type").arg(file);
            c
        } else {
            let mut c = Command::new("cat");
            c.arg(file);
            c
        };
        c.stdout(out).stderr(Stdio::null()).spawn()
    }

    /// GitHub naming 99.0.0, and this build's version, the latest.
    fn names_99(out: File) -> io::Result<Child> {
        print(canned("99"), out)
    }

    fn names_this(out: File) -> io::Result<Child> {
        print(canned("this"), out)
    }

    /// Something that fails, as curl does on an HTTP error.
    fn failing(out: File) -> io::Result<Child> {
        let mut c = if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.args(["/C", "exit 22"]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", "exit 22"]);
            c
        };
        c.stdout(out).spawn()
    }

    /// Something that takes far longer than anyone waits.
    fn stuck(out: File) -> io::Result<Child> {
        let mut c = if cfg!(windows) {
            let mut c = Command::new("ping");
            c.args(["-n", "120", "127.0.0.1"]);
            c
        } else {
            let mut c = Command::new("sleep");
            c.arg("120");
            c
        };
        c.stdout(out).spawn()
    }

    /// The frames until the check is over (10 s at most).
    fn settle(u: &mut Update) {
        let t0 = Instant::now();
        while matches!(u.state, State::Checking(_)) {
            assert!(t0.elapsed() < Duration::from_secs(10), "still checking");
            u.tick();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn file_of(u: &Update) -> PathBuf {
        match &u.state {
            State::Checking(c) => c.file.clone(),
            _ => panic!("not checking"),
        }
    }

    /// A newer release: its version shown, DOWNLOAD opening its installer, and what to do
    /// with it; the answer's file gone once read.
    #[test]
    fn a_newer_release_offers_its_installer() {
        std::fs::write(canned("99"), answer("v99.0.0")).unwrap();
        let mut u = Update::with(names_99, browser);
        let s = u.scene();
        assert_eq!(
            (s.text.as_str(), s.button.as_str()),
            (format!("CA-72 {VERSION}").as_str(), "CHECK FOR UPDATES")
        );
        u.press();
        assert_eq!(u.scene().button, "CHECKING…");
        let file = file_of(&u);
        // Pressed again while it checks: nothing more.
        u.press();
        assert_eq!(file_of(&u), file);
        settle(&mut u);
        assert!(!file.exists(), "{file:?} left behind");
        let s = u.scene();
        assert_eq!(
            s,
            UpdateScene {
                text: format!("99.0.0 IS OUT (THIS IS {VERSION})"),
                tone: Tone::News,
                button: "DOWNLOAD".into(),
            }
        );
        u.press();
        let want = match INSTALLER {
            Some(i) => format!("{RELEASES}/download/v99.0.0/CA-72-99.0.0-{i}"),
            None => format!("{RELEASES}/tag/v99.0.0"),
        };
        assert_eq!(opened(), vec![want]);
        assert_eq!(u.scene().text, "CLOSE THE DAW, THEN INSTALL IT");
        // A browser that will not open says so.
        u.open = no_browser;
        u.press();
        assert_eq!(
            (u.scene().text.as_str(), u.scene().tone),
            ("NO BROWSER WOULD OPEN", Tone::Trouble)
        );
        let _ = std::fs::remove_file(canned("99"));
    }

    /// This build's release the latest: up to date, and it may be asked again.
    #[test]
    fn the_latest_release_is_up_to_date() {
        std::fs::write(canned("this"), answer(&format!("v{VERSION}"))).unwrap();
        let mut u = Update::with(names_this, browser);
        u.press();
        settle(&mut u);
        assert_eq!(
            u.scene(),
            UpdateScene {
                text: format!("{VERSION} IS UP TO DATE"),
                tone: Tone::Dim,
                button: "CHECK FOR UPDATES".into(),
            }
        );
        // Asked again: checked again.
        u.press();
        assert_eq!(u.scene().button, "CHECKING…");
        settle(&mut u);
        assert_eq!(u.scene().text, format!("{VERSION} IS UP TO DATE"));
        assert!(opened().is_empty());
        let _ = std::fs::remove_file(canned("this"));
    }

    /// curl missing, or failing: the releases' page offered instead.
    #[test]
    fn a_failed_check_offers_the_releases_page() {
        for fetch in [no_curl as Fetch, failing] {
            let mut u = Update::with(fetch, browser);
            u.press();
            settle(&mut u);
            assert_eq!(
                u.scene(),
                UpdateScene {
                    text: "COULD NOT CHECK".into(),
                    tone: Tone::Trouble,
                    button: "RELEASES PAGE".into(),
                }
            );
            u.press();
            assert_eq!(opened(), vec![RELEASES.to_owned()]);
        }
    }

    /// The editor closed while curl works: curl ended at once (not waited for), its file
    /// gone.
    #[test]
    fn closing_the_editor_ends_curl() {
        let mut u = Update::with(stuck, browser);
        u.press();
        let file = file_of(&u);
        std::thread::sleep(Duration::from_millis(200));
        u.tick();
        assert!(matches!(u.state, State::Checking(_)));
        let t0 = Instant::now();
        drop(u);
        assert!(t0.elapsed() < Duration::from_secs(5), "{:?}", t0.elapsed());
        assert!(!file.exists(), "{file:?} left behind");
    }

    /// An opener that exits with `code`, as `xdg-open` does.
    fn exiting(code: u8) -> io::Result<Option<Child>> {
        let mut c = if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(format!("exit {code}"));
            c
        } else {
            let mut c = Command::new("sh");
            c.arg("-c").arg(format!("exit {code}"));
            c
        };
        c.spawn().map(Some)
    }

    fn opener_fails(_url: &str) -> io::Result<Option<Child>> {
        exiting(4)
    }

    fn opener_succeeds(_url: &str) -> io::Result<Option<Child>> {
        exiting(0)
    }

    /// An opener that fails once started (`xdg-open` with no browser, or no display): no
    /// browser opened, said once it has ended; one that ends well changes nothing.
    #[test]
    fn an_opener_that_fails_opens_no_browser() {
        for (open, text) in [
            (opener_succeeds as Open, "COULD NOT CHECK"),
            (opener_fails, "NO BROWSER WOULD OPEN"),
        ] {
            let mut u = Update::with(no_curl, open);
            // The check fails at once; RELEASES PAGE.
            u.press();
            u.press();
            let t0 = Instant::now();
            while !u.openers.is_empty() {
                assert!(t0.elapsed() < Duration::from_secs(10), "still opening");
                u.tick();
                std::thread::sleep(Duration::from_millis(5));
            }
            assert_eq!(u.scene().text, text);
        }
    }

    /// GitHub asked by the system's curl (`cargo test -p ca72-plugin -- --ignored
    /// github_names_its_latest_release`; the network's).
    #[test]
    #[ignore = "asks GitHub"]
    fn github_names_its_latest_release() {
        let mut u = Update::with(curl, browser);
        u.press();
        let t0 = Instant::now();
        while matches!(u.state, State::Checking(_)) {
            assert!(t0.elapsed() < PATIENCE + Duration::from_secs(1));
            u.tick();
            std::thread::sleep(Duration::from_millis(20));
        }
        eprintln!("{:?}", u.scene());
        assert!(matches!(u.state, State::Latest | State::Newer(_)), "{u:?}");
    }

    /// `url` opened in this system's browser as DOWNLOAD and RELEASES PAGE open it: the
    /// opener (macOS's `open`, `xdg-open`) ended well, or runs on (the browser itself).
    fn opens(url: &str) {
        let opener = browse(url).expect("a browser opened");
        if let Some(mut o) = opener {
            let t0 = Instant::now();
            while t0.elapsed() < Duration::from_secs(15) {
                if let Some(status) = o.try_wait().unwrap() {
                    assert!(status.success(), "{status}");
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    /// The releases' page opened in this system's browser (`cargo test -p ca72-plugin --
    /// --ignored the_browser_opens_the_releases_page`).
    #[test]
    #[ignore = "opens the browser"]
    fn the_browser_opens_the_releases_page() {
        opens(RELEASES);
    }

    /// GitHub asked by the system's curl, and the latest release's installer for this system
    /// downloaded by the browser, as DOWNLOAD does a newer one's (`cargo test -p ca72-plugin
    /// -- --ignored the_browser_downloads_this_systems_installer`).
    #[test]
    #[ignore = "asks GitHub and downloads an installer"]
    fn the_browser_downloads_this_systems_installer() {
        let mut c = Check::start(curl).unwrap();
        let t0 = Instant::now();
        let answer = loop {
            match c.poll() {
                Poll::Waiting => {
                    assert!(t0.elapsed() < PATIENCE + Duration::from_secs(1));
                    std::thread::sleep(Duration::from_millis(20));
                }
                Poll::Answered(a) => break a,
                Poll::Failed => panic!("no answer from GitHub"),
            }
        };
        let r = release(&answer, INSTALLER).expect("a release");
        eprintln!("{r:?}");
        if let Some(i) = INSTALLER {
            assert!(r.download.ends_with(&format!("{}-{i}", r.version)), "{r:?}");
        }
        opens(&r.download);
    }
}
