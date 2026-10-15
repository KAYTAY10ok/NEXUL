//! NEXUL Studio — native desktop application (GPUI and gpui-component).
//!
//! `NEXUL-Studio-Native [folder | file]`. The services (files, watch, runs, compiler)
//! come from the `nexul_studio` library, shared with the web interface.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod explorer;
mod settings;
mod studio;

use gpui_kit::component::GlobalState;
use gpui_kit::*;
use std::path::PathBuf;

actions!(
    studio,
    [OpenFolder, Save, SaveAll, CloseEditor, ToggleSidebar, TogglePanel, ToggleMinimal, ToggleTheme, Run, Stop, NextTab, Quit]
);

/// Shortcuts, close to those of Visual Studio Code.
fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-o", OpenFolder, None),
        KeyBinding::new("ctrl-s", Save, None),
        KeyBinding::new("ctrl-shift-s", SaveAll, None),
        KeyBinding::new("ctrl-w", CloseEditor, None),
        KeyBinding::new("ctrl-f4", CloseEditor, None),
        KeyBinding::new("ctrl-b", ToggleSidebar, None),
        KeyBinding::new("ctrl-j", TogglePanel, None),
        KeyBinding::new("ctrl-k z", ToggleMinimal, None),
        KeyBinding::new("f5", Run, None),
        KeyBinding::new("shift-f5", Stop, None),
        KeyBinding::new("ctrl-tab", NextTab, None),
        KeyBinding::new("alt-f4", Quit, None),
    ]);
}

/// The menus: in the title bar on Windows and Linux, in the menu bar of macOS.
fn menus() -> Vec<Menu> {
    vec![
        Menu::new("Fichier").items([
            MenuItem::action("Ouvrir un dossier…", OpenFolder),
            MenuItem::separator(),
            MenuItem::action("Enregistrer", Save),
            MenuItem::action("Tout enregistrer", SaveAll),
            MenuItem::separator(),
            MenuItem::action("Fermer l'éditeur", CloseEditor),
            MenuItem::separator(),
            MenuItem::action("Quitter", Quit),
        ]),
        Menu::new("Affichage").items([
            MenuItem::action("Explorateur", ToggleSidebar),
            MenuItem::action("Sortie", TogglePanel),
            MenuItem::action("Mode minimaliste", ToggleMinimal),
            MenuItem::separator(),
            MenuItem::action("Thème clair ou sombre", ToggleTheme),
        ]),
        Menu::new("Exécution").items([MenuItem::action("Exécuter", Run), MenuItem::action("Arrêter", Stop)]),
    ]
}

fn set_menus(cx: &mut App) {
    GlobalState::global_mut(cx).set_app_menus(menus().into_iter().map(|m| m.owned()).collect());
    cx.set_menus(menus());
}

fn main() {
    let start = std::env::args().nth(1).map(PathBuf::from);
    // The icons (Lucide) are embedded in the program.
    gpui_kit::application().with_assets(gpui_kit::assets::Assets).run(move |cx| {
        gpui_kit::init(cx);
        bind_keys(cx);
        set_menus(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(480.), px(320.))),
            ..gpui_kit::component::TitleBar::window_options()
        };
        let (_, view) = gpui_kit::open_window(options, cx, move |window, cx| cx.new(|cx| studio::Studio::new(start, window, cx))).expect("cannot open the window");
        // Closing the window ends the application.
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        let _ = view;
    });
}
