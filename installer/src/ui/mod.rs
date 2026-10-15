//! The native window of the installer and of the uninstaller (step 2 of
//! `installer/docs/etude-interface-native.md`): a wizard with the controls of Windows, which
//! replaces the HTML page shown in Edge (still available with `--web-ui`).

pub mod model;
#[cfg(feature = "test-sandbox")]
mod selftest;
mod sys;
mod window;

pub use window::run;
