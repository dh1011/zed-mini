use std::path::PathBuf;

use anyhow::Result;
use workspace::{ItemId, WorkspaceId};

pub struct EditorDb;

pub static DB: EditorDb = EditorDb;

impl EditorDb {
    pub fn get_path(
        &self,
        _item_id: ItemId,
        _workspace_id: WorkspaceId,
    ) -> Result<Option<PathBuf>> {
        Ok(None)
    }

    pub async fn save_path(
        &self,
        _item_id: ItemId,
        _workspace_id: WorkspaceId,
        _path: PathBuf,
    ) -> Result<()> {
        Ok(())
    }

    pub fn get_scroll_position(
        &self,
        _item_id: ItemId,
        _workspace_id: WorkspaceId,
    ) -> Result<Option<(u32, f32, f32)>> {
        Ok(None)
    }

    pub async fn save_scroll_position(
        &self,
        _item_id: ItemId,
        _workspace_id: WorkspaceId,
        _top_row: u32,
        _vertical_offset: f32,
        _horizontal_offset: f32,
    ) -> Result<()> {
        Ok(())
    }
}
