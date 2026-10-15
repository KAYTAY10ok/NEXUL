//! The self-test of the window (feature `test-sandbox` only): `--ui-selftest <folder>`
//! drives the wizard from start to end as a user would (accept the licenses, back up the
//! earlier versions, uncheck the examples, type the folder, install; or uninstall), checks
//! the state of the buttons at each step, saves a screenshot of each page in the folder
//! (`.bmp`), and ends the program with code 0 if all went well, 1 otherwise (the reason on
//! the error output). The folder to type comes from `NEXUL_SETUP_SELFTEST_DIR`.

use super::model::Page;
use super::sys::*;
use super::window::{child_by_id, quit, ALL_BACKUP_ID};
use crate::win::ffi::HWND;
use crate::win::wide;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct SelfTest {
    shots: PathBuf,
    target: String,
    step: u32,
    start: Instant,
    progress_shot: bool,
    /// When the window was shown again after being reduced.
    restored: Option<Instant>,
}

impl SelfTest {
    pub fn new(shots: PathBuf) -> SelfTest {
        SelfTest {
            shots,
            target: std::env::var("NEXUL_SETUP_SELFTEST_DIR").unwrap_or_default(),
            step: 0,
            start: Instant::now(),
            progress_shot: false,
            restored: None,
        }
    }
}

/// What the self-test does after a decision, outside of the application.
pub enum Act {
    Click(HWND),
    SetText(HWND, String),
    Shot(PathBuf),
    Fail(String),
    Pass,
}

fn enabled(h: Option<HWND>) -> bool {
    h.is_some_and(|h| unsafe { IsWindowEnabled(h) } != 0)
}

/// Decides the next actions from the state of the wizard.
pub fn plan(app: &mut super::window::App) -> Vec<Act> {
    let page = app.page();
    let next = app.next_button();
    let accept = app.accept_box();
    let samples = app.component_box("samples");
    let samples_on = app.model().is_chosen("samples");
    let scanned = app.model().old.is_some();
    let uninstall = app.model().uninstall;
    let dir = app.model().dir.clone();
    let dir_box = app.dir_box();
    let error = app.error();
    let all_backup = child_by_id(app, ALL_BACKUP_ID);
    let hwnd = app.hwnd();
    let Some(t) = app.selftest() else { return Vec::new() };
    if t.start.elapsed() > Duration::from_secs(120) {
        return vec![Act::Fail(format!("délai dépassé à l'étape {}", t.step))];
    }
    // Reduced in the meantime (« Afficher le bureau », Windows + D…): a reduced window has no
    // client area to capture. Shown again where it was, out of the screen, without taking
    // the focus; the next steps wait until it is painted again.
    if unsafe { IsIconic(hwnd) } != 0 {
        unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
        t.restored = Some(Instant::now());
        return Vec::new();
    }
    if t.restored.is_some_and(|r| r.elapsed() < Duration::from_millis(500)) {
        return Vec::new();
    }
    let shots = t.shots.clone();
    let shot = |name: &str| Act::Shot(shots.join(format!("{name}.bmp")));
    fn go(t: &mut SelfTest, step: u32, acts: Vec<Act>) -> Vec<Act> {
        t.step = step;
        acts
    }
    if uninstall {
        return match t.step {
            0 if page == Page::Uninstall => go(t, 1, vec![shot("1-desinstallation"), Act::Click(next.unwrap())]),
            1 if page == Page::Progress && !t.progress_shot => {
                t.progress_shot = true;
                vec![shot("2-desinstallation-en-cours")]
            }
            1 if page == Page::Done => go(t, 2, vec![shot("3-termine")]),
            2 => match error {
                Some(e) => vec![Act::Fail(format!("la désinstallation a échoué : {e}"))],
                None => go(t, 3, vec![Act::Pass, Act::Click(next.unwrap())]),
            },
            _ => Vec::new(),
        };
    }
    match t.step {
        // Welcome: « Suivant » waits for the licenses and the search of earlier versions.
        0 if scanned => {
            if enabled(next) {
                vec![Act::Fail("« Suivant » actif avant l'acceptation des licences".into())]
            } else {
                go(t, 1, vec![shot("1-bienvenue"), Act::Click(accept.unwrap())])
            }
        }
        1 => {
            if enabled(next) {
                go(t, 2, vec![Act::Click(next.unwrap())])
            } else {
                vec![Act::Fail("« Suivant » inactif après l'acceptation des licences".into())]
            }
        }
        2 => match page {
            Page::OldVersions if enabled(next) => vec![Act::Fail("« Suivant » actif avant les choix".into())],
            Page::OldVersions => match all_backup {
                Some(b) => go(t, 3, vec![shot("2-anciennes-versions"), Act::Click(b)]),
                None => vec![Act::Fail("bouton « Tout sauvegarder » absent".into())],
            },
            Page::Components => go(t, 4, Vec::new()),
            _ => Vec::new(),
        },
        3 => {
            if enabled(next) {
                go(t, 4, vec![Act::Click(next.unwrap())])
            } else {
                vec![Act::Fail("« Suivant » inactif après « Tout sauvegarder »".into())]
            }
        }
        4 if page == Page::Components => match samples {
            Some(s) if samples_on => go(t, 5, vec![shot("3-composants"), Act::Click(s)]),
            _ => go(t, 5, vec![shot("3-composants")]),
        },
        5 => {
            if samples.is_some() && samples_on {
                vec![Act::Fail("les exemples sont restés cochés".into())]
            } else {
                go(t, 6, vec![Act::Click(next.unwrap())])
            }
        }
        6 if page == Page::Location => {
            let target = t.target.clone();
            go(t, 7, vec![Act::SetText(dir_box.unwrap(), target)])
        }
        7 => {
            if dir != t.target {
                vec![Act::Fail(format!("dossier non pris en compte : {dir:?}"))]
            } else {
                go(t, 8, vec![shot("4-emplacement"), Act::Click(next.unwrap())])
            }
        }
        8 if page == Page::Progress && !t.progress_shot => {
            t.progress_shot = true;
            vec![shot("5-installation")]
        }
        8 if page == Page::Done => go(t, 9, vec![shot("6-termine")]),
        9 => match error {
            Some(e) => vec![Act::Fail(format!("l'installation a échoué : {e}"))],
            None => go(t, 10, vec![Act::Pass, Act::Click(next.unwrap())]),
        },
        _ => Vec::new(),
    }
}

/// Does the actions (the application is available again: the clicks are handled).
pub fn perform(hwnd: HWND, acts: Vec<Act>) {
    for act in acts {
        match act {
            Act::Click(h) => unsafe {
                SendMessageW(h, BM_CLICK, 0, 0);
            },
            Act::SetText(h, text) => unsafe {
                SetWindowTextW(h, wide(&text).as_ptr());
            },
            Act::Shot(path) => {
                if let Err(e) = screenshot(hwnd, &path) {
                    eprintln!("UI-SELFTEST: capture {} : {e}", path.display());
                    return quit(hwnd, 1);
                }
                // `NEXUL_SETUP_SELFTEST_MINIMIZE`: the window is reduced after each capture, as
                // « Afficher le bureau » would do while the tests run.
                if std::env::var_os("NEXUL_SETUP_SELFTEST_MINIMIZE").is_some() {
                    unsafe { ShowWindow(hwnd, SW_MINIMIZE) };
                }
            }
            Act::Fail(why) => {
                eprintln!("UI-SELFTEST: {why}");
                return quit(hwnd, 1);
            }
            Act::Pass => println!("UI-SELFTEST OK"),
        }
    }
}

/// The client area of the window, as rendered, in a 32-bit `.bmp` file.
fn screenshot(hwnd: HWND, path: &std::path::Path) -> Result<(), String> {
    let mut r = RECT::default();
    unsafe { GetClientRect(hwnd, &mut r) };
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    if w <= 0 || h <= 0 {
        return Err("fenêtre vide".into());
    }
    let header = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w,
        biHeight: -h, // rows from the top
        biPlanes: 1,
        biBitCount: 32,
        biCompression: 0,
        biSizeImage: 0,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let pixels = unsafe {
        let dc = CreateCompatibleDC(std::ptr::null_mut());
        let bmp = CreateDIBSection(dc, &header, 0, &mut bits, std::ptr::null_mut(), 0);
        if bmp.is_null() || bits.is_null() {
            DeleteDC(dc);
            return Err("image impossible à créer".into());
        }
        let old = SelectObject(dc, bmp);
        // Client area only, with the content as the compositor renders it.
        let ok = PrintWindow(hwnd, dc, 1 | PW_RENDERFULLCONTENT);
        let data = std::slice::from_raw_parts(bits as *const u8, (w * h * 4) as usize).to_vec();
        SelectObject(dc, old);
        DeleteObject(bmp);
        DeleteDC(dc);
        if ok == 0 {
            return Err("PrintWindow a échoué".into());
        }
        data
    };
    let mut file = Vec::with_capacity(54 + pixels.len());
    file.extend_from_slice(b"BM");
    file.extend_from_slice(&(54 + pixels.len() as u32).to_le_bytes());
    file.extend_from_slice(&[0, 0, 0, 0]);
    file.extend_from_slice(&54u32.to_le_bytes());
    file.extend_from_slice(&40u32.to_le_bytes());
    file.extend_from_slice(&w.to_le_bytes());
    file.extend_from_slice(&(-h).to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&32u16.to_le_bytes());
    file.extend_from_slice(&[0; 24]);
    file.extend_from_slice(&pixels);
    std::fs::write(path, file).map_err(|e| e.to_string())
}
