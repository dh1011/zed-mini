use client::{proto, User};
use collections::HashMap;
use std::{fmt, sync::Arc};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ParticipantLocation {
    SharedProject { project_id: u64 },
    UnsharedProject,
    External,
}

#[derive(Clone, Debug)]
pub struct RemoteParticipant {
    pub user: Arc<User>,
    pub peer_id: proto::PeerId,
    pub projects: Vec<proto::ParticipantProject>,
    pub location: ParticipantLocation,
    pub tracks: HashMap<String, Arc<RemoteVideoTrack>>,
}

#[derive(Clone)]
pub struct RemoteVideoTrack;

impl fmt::Debug for RemoteVideoTrack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteVideoTrack").finish()
    }
}
