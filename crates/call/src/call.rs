pub mod participant;
pub mod room;

use std::sync::Arc;

use anyhow::{anyhow, Result};
use client::{proto, Client, User, UserStore};
use gpui::{AppContext, Entity, ModelContext, ModelHandle, MutableAppContext, Task};
use postage::watch;
use project::Project;

pub use participant::ParticipantLocation;
pub use room::Room;

pub fn init(_client: Arc<Client>, _user_store: ModelHandle<UserStore>, cx: &mut MutableAppContext) {
    let active_call = cx.add_model(ActiveCall::new);
    cx.set_global(active_call);
}

#[derive(Clone)]
pub struct IncomingCall {
    pub room_id: u64,
    pub calling_user: Arc<User>,
    pub participants: Vec<Arc<User>>,
    pub initial_project: Option<proto::ParticipantProject>,
}

pub struct ActiveCall {
    incoming_call: (
        watch::Sender<Option<IncomingCall>>,
        watch::Receiver<Option<IncomingCall>>,
    ),
}

impl Entity for ActiveCall {
    type Event = room::Event;
}

impl ActiveCall {
    fn new(_cx: &mut ModelContext<Self>) -> Self {
        Self {
            incoming_call: watch::channel(),
        }
    }

    pub fn global(cx: &AppContext) -> ModelHandle<Self> {
        cx.global::<ModelHandle<Self>>().clone()
    }

    pub fn invite(
        &mut self,
        _called_user_id: u64,
        _initial_project: Option<ModelHandle<Project>>,
        _cx: &mut ModelContext<Self>,
    ) -> Task<Result<()>> {
        Task::ready(Err(anyhow!("collaboration is disabled in this build")))
    }

    pub fn cancel_invite(
        &mut self,
        _called_user_id: u64,
        _cx: &mut ModelContext<Self>,
    ) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }

    pub fn incoming(&self) -> watch::Receiver<Option<IncomingCall>> {
        self.incoming_call.1.clone()
    }

    pub fn accept_incoming(&mut self, _cx: &mut ModelContext<Self>) -> Task<Result<()>> {
        Task::ready(Err(anyhow!("collaboration is disabled in this build")))
    }

    pub fn decline_incoming(&mut self) -> Result<()> {
        self.incoming_call.0.borrow_mut().take();
        Ok(())
    }

    pub fn hang_up(&mut self, _cx: &mut ModelContext<Self>) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }

    pub fn share_project(
        &mut self,
        _project: ModelHandle<Project>,
        _cx: &mut ModelContext<Self>,
    ) -> Task<Result<()>> {
        Task::ready(Err(anyhow!("collaboration is disabled in this build")))
    }

    pub fn unshare_project(
        &mut self,
        _project: ModelHandle<Project>,
        _cx: &mut ModelContext<Self>,
    ) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }

    pub fn set_location(
        &mut self,
        _project: Option<&ModelHandle<Project>>,
        _cx: &mut ModelContext<Self>,
    ) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }

    pub fn room(&self) -> Option<&ModelHandle<Room>> {
        None
    }
}
