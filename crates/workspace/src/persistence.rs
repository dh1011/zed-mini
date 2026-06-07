pub mod model;

use std::{
    path::Path,
    sync::atomic::{AtomicI64, Ordering},
};

use anyhow::Result;
use gpui::WindowBounds;
use uuid::Uuid;

use crate::WorkspaceId;

use model::{SerializedWorkspace, WorkspaceLocation};

pub struct WorkspaceDb;

pub static DB: WorkspaceDb = WorkspaceDb;

static NEXT_WORKSPACE_ID: AtomicI64 = AtomicI64::new(1);

impl WorkspaceDb {
    pub fn workspace_for_roots<P: AsRef<Path>>(
        &self,
        _worktree_roots: &[P],
    ) -> Option<SerializedWorkspace> {
        None
    }

    pub async fn save_workspace(&self, _workspace: SerializedWorkspace) {}

    pub async fn next_id(&self) -> Result<WorkspaceId> {
        Ok(NEXT_WORKSPACE_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub async fn recent_workspaces_on_disk(&self) -> Result<Vec<(WorkspaceId, WorkspaceLocation)>> {
        Ok(Vec::new())
    }

    pub async fn last_workspace(&self) -> Result<Option<WorkspaceLocation>> {
        Ok(None)
    }

    pub async fn update_timestamp(&self, _workspace_id: WorkspaceId) -> Result<()> {
        Ok(())
    }

    pub async fn set_window_bounds(
        &self,
        _workspace_id: WorkspaceId,
        _bounds: WindowBounds,
        _display: Uuid,
    ) -> Result<()> {
        Ok(())
    }
}
