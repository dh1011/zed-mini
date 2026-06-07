use crate::participant::RemoteParticipant;
use client::{
    proto::{self, PeerId},
    User,
};
use collections::BTreeMap;
use gpui::Entity;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    ParticipantLocationChanged {
        participant_id: proto::PeerId,
    },
    RemoteVideoTracksChanged {
        participant_id: proto::PeerId,
    },
    RemoteProjectShared {
        owner: Arc<User>,
        project_id: u64,
        worktree_root_names: Vec<String>,
    },
    RemoteProjectUnshared {
        project_id: u64,
    },
    Left,
}

pub struct Room {
    remote_participants: BTreeMap<u64, RemoteParticipant>,
    pending_participants: Vec<Arc<User>>,
}

impl Entity for Room {
    type Event = Event;
}

impl Room {
    pub fn remote_participant_for_peer_id(&self, _peer_id: PeerId) -> Option<&RemoteParticipant> {
        None
    }

    pub fn remote_participants(&self) -> &BTreeMap<u64, RemoteParticipant> {
        &self.remote_participants
    }

    pub fn pending_participants(&self) -> &[Arc<User>] {
        &self.pending_participants
    }

    pub fn is_screen_sharing(&self) -> bool {
        false
    }
}
