use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use async_recursion::async_recursion;
use gpui::{AsyncAppContext, Axis, ModelHandle, Task, ViewHandle, WindowBounds};
use project::Project;
use util::ResultExt;
use uuid::Uuid;

use crate::{
    dock::DockPosition, ItemDeserializers, Member, Pane, PaneAxis, Workspace, WorkspaceId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceLocation(Arc<Vec<PathBuf>>);

impl WorkspaceLocation {
    pub fn paths(&self) -> Arc<Vec<PathBuf>> {
        self.0.clone()
    }
}

impl<P: AsRef<Path>, T: IntoIterator<Item = P>> From<T> for WorkspaceLocation {
    fn from(iterator: T) -> Self {
        let mut roots = iterator
            .into_iter()
            .map(|p| p.as_ref().to_path_buf())
            .collect::<Vec<_>>();
        roots.sort();
        Self(Arc::new(roots))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct SerializedWorkspace {
    pub id: WorkspaceId,
    pub location: WorkspaceLocation,
    pub dock_position: DockPosition,
    pub center_group: SerializedPaneGroup,
    pub dock_pane: SerializedPane,
    pub left_sidebar_open: bool,
    pub bounds: Option<WindowBounds>,
    pub display: Option<Uuid>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SerializedPaneGroup {
    Group {
        axis: Axis,
        children: Vec<SerializedPaneGroup>,
    },
    Pane(SerializedPane),
}

#[cfg(test)]
impl Default for SerializedPaneGroup {
    fn default() -> Self {
        Self::Pane(SerializedPane {
            children: vec![SerializedItem::default()],
            active: false,
        })
    }
}

impl SerializedPaneGroup {
    #[async_recursion(?Send)]
    pub(crate) async fn deserialize(
        &self,
        project: &ModelHandle<Project>,
        workspace_id: WorkspaceId,
        workspace: &ViewHandle<Workspace>,
        cx: &mut AsyncAppContext,
    ) -> Option<(Member, Option<ViewHandle<Pane>>)> {
        match self {
            SerializedPaneGroup::Group { axis, children } => {
                let mut current_active_pane = None;
                let mut members = Vec::new();
                for child in children {
                    if let Some((new_member, active_pane)) = child
                        .deserialize(project, workspace_id, workspace, cx)
                        .await
                    {
                        members.push(new_member);
                        current_active_pane = current_active_pane.or(active_pane);
                    }
                }

                if members.is_empty() {
                    return None;
                }

                if members.len() == 1 {
                    return Some((members.remove(0), current_active_pane));
                }

                Some((
                    Member::Axis(PaneAxis {
                        axis: *axis,
                        members,
                    }),
                    current_active_pane,
                ))
            }
            SerializedPaneGroup::Pane(serialized_pane) => {
                let pane = workspace.update(cx, |workspace, cx| workspace.add_pane(cx));
                let active = serialized_pane.active;
                serialized_pane
                    .deserialize_to(project, &pane, workspace_id, workspace, cx)
                    .await;

                if pane.read_with(cx, |pane, _| pane.items_len() != 0) {
                    Some((Member::Pane(pane.clone()), active.then(|| pane)))
                } else {
                    workspace.update(cx, |workspace, cx| workspace.remove_pane(pane, cx));
                    None
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub struct SerializedPane {
    pub(crate) active: bool,
    pub(crate) children: Vec<SerializedItem>,
}

impl SerializedPane {
    pub fn new(children: Vec<SerializedItem>, active: bool) -> Self {
        SerializedPane { children, active }
    }

    pub async fn deserialize_to(
        &self,
        project: &ModelHandle<Project>,
        pane_handle: &ViewHandle<Pane>,
        workspace_id: WorkspaceId,
        workspace: &ViewHandle<Workspace>,
        cx: &mut AsyncAppContext,
    ) {
        let mut active_item_index = None;
        for (index, item) in self.children.iter().enumerate() {
            let project = project.clone();
            let item_handle = pane_handle
                .update(cx, |_, cx| {
                    if let Some(deserializer) = cx.global::<ItemDeserializers>().get(&item.kind) {
                        deserializer(
                            project,
                            workspace.downgrade(),
                            workspace_id,
                            item.item_id,
                            cx,
                        )
                    } else {
                        Task::ready(Err(anyhow::anyhow!(
                            "Deserializer does not exist for item kind: {}",
                            item.kind
                        )))
                    }
                })
                .await
                .log_err();

            if let Some(item_handle) = item_handle {
                workspace.update(cx, |workspace, cx| {
                    Pane::add_item(workspace, &pane_handle, item_handle, false, false, None, cx);
                })
            }

            if item.active {
                active_item_index = Some(index);
            }
        }

        if let Some(active_item_index) = active_item_index {
            pane_handle.update(cx, |pane, cx| {
                pane.activate_item(active_item_index, false, false, cx);
            })
        }
    }
}

pub type GroupId = i64;
pub type PaneId = i64;
pub type ItemId = usize;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SerializedItem {
    pub kind: Arc<str>,
    pub item_id: ItemId,
    pub active: bool,
}

impl SerializedItem {
    pub fn new(kind: impl AsRef<str>, item_id: ItemId, active: bool) -> Self {
        Self {
            kind: Arc::from(kind.as_ref()),
            item_id,
            active,
        }
    }
}

#[cfg(test)]
impl Default for SerializedItem {
    fn default() -> Self {
        SerializedItem {
            kind: Arc::from("Terminal"),
            item_id: 100000,
            active: false,
        }
    }
}
