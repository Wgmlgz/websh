use std::sync::Arc;

use virtual_display::VirtualDisplayManager;
use webrtc::{api::API, peer_connection::configuration::RTCConfiguration};

use crate::{peer::PeerMap, shell::SessionMap, signal::Signaling};

pub struct State<S: Signaling> {
    pub api: API,
    pub config: RTCConfiguration,
    pub session_map: SessionMap,
    pub my_name: String,
    pub signaling: Arc<S>,
    pub peer_map: PeerMap,
    pub display_manager: Arc<VirtualDisplayManager>,
}
