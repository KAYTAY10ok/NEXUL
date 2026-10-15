//! The icons embedded in the program: those of the components, and a few more (Lucide).

use gpui_kit::{AssetSource, Result, SharedString};
use std::borrow::Cow;

gpui_kit::assets::icon_assets!(
    StudioIcons,
    [FilePlus, FolderPlus, ListChevronsDownUp, Scissors, ClipboardPaste, Trash, Pencil, FolderSearch]
);

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = StudioIcons.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(StudioIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
