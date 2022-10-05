use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
    time::Duration,
};

use ahash::AHashMap;
use cid::Cid;
use derivative::Derivative;
use libp2p::PeerId;

use crate::{network::Network, Block};

use super::{
    block_presence_manager::BlockPresenceManager, peer_manager::PeerManager,
    provider_query_manager::ProviderQueryManager, session::Session,
    session_interest_manager::SessionInterestManager, session_peer_manager::SessionPeerManager,
};

#[derive(Debug, Clone)]
pub struct SessionManager {
    inner: Arc<Inner>,
}

#[derive(Derivative)]
#[derivative(Debug)]
struct Inner {
    self_id: PeerId,
    session_interest_manager: SessionInterestManager,
    block_presence_manager: BlockPresenceManager,
    peer_manager: PeerManager,
    provider_finder: ProviderQueryManager,
    sessions: RwLock<AHashMap<u64, Session>>,
    session_index: AtomicU64,
    network: Network,
    #[derivative(Debug = "ignore")]
    notify: bus::BusReadHandle<Block>,
}

impl SessionManager {
    pub fn new(
        self_id: PeerId,
        session_interest_manager: SessionInterestManager,
        block_presence_manager: BlockPresenceManager,
        peer_manager: PeerManager,
        provider_finder: ProviderQueryManager,
        network: Network,
        notify: bus::BusReadHandle<Block>,
    ) -> Self {
        SessionManager {
            inner: Arc::new(Inner {
                self_id,
                session_interest_manager,
                block_presence_manager,
                peer_manager,
                provider_finder,
                sessions: Default::default(),
                session_index: Default::default(),
                network,
                notify,
            }),
        }
    }

    /// Initializes a new session and starts tracking it.
    pub fn new_session(
        &self,
        provider_search_delay: Duration,
        rebroadcast_delay: Duration,
    ) -> Session {
        let id = self.get_next_session_id();
        let session_peer_manager = SessionPeerManager::new(id, self.inner.network.clone());

        let session = Session::new(
            self.inner.self_id,
            id,
            self.clone(),
            self.inner.peer_manager.clone(),
            session_peer_manager,
            self.inner.provider_finder.clone(),
            self.inner.session_interest_manager.clone(),
            self.inner.block_presence_manager.clone(),
            self.inner.provider_finder.clone(),
            self.inner.notify.clone(),
            provider_search_delay,
            rebroadcast_delay,
        );

        self.inner
            .sessions
            .write()
            .unwrap()
            .insert(id, session.clone());
        session
    }

    pub fn remove_session(&self, session_id: u64) {
        let cancels = self
            .inner
            .session_interest_manager
            .remove_session(session_id);
        self.cancel_wants(&cancels);

        self.inner.sessions.write().unwrap().remove(&session_id);
    }

    /// Returns the next sequential identifier for a session.
    pub fn get_next_session_id(&self) -> u64 {
        self.inner.session_index.fetch_add(1, Ordering::SeqCst)
    }

    pub fn receive_from(
        &self,
        peer: Option<PeerId>,
        blocks: &[Cid],
        haves: &[Cid],
        dont_haves: &[Cid],
    ) {
        // Record block presence for HAVE/DONT_HAVE.
        if let Some(ref peer) = peer {
            self.inner
                .block_presence_manager
                .receive_from(peer, haves, dont_haves);
        }

        for id in &self
            .inner
            .session_interest_manager
            .interested_sessions(blocks, haves, dont_haves)
        {
            let sessions = &*self.inner.sessions.read().unwrap();
            if let Some(session) = sessions.get(id) {
                session.receive_from(peer, blocks, haves, dont_haves);
            }
        }

        // Send CANCELs to all peers with want-have/want-block
        self.inner.peer_manager.send_cancels(blocks);
    }

    pub fn cancel_session_wants(&self, session_id: u64, wants: &[Cid]) {
        // Remove session's interest in the given blocks - returns the keys taht
        // no session is interested in anymore.
        let cancels = self
            .inner
            .session_interest_manager
            .remove_session_interested(session_id, wants);
        self.cancel_wants(&cancels);
    }

    fn cancel_wants(&self, wants: &[Cid]) {
        // Free up block presence tracking
        self.inner.block_presence_manager.remove_keys(wants);

        // Send CANCEL to all peers for blocks that no session is interested anymore.
        self.inner.peer_manager.send_cancels(wants);
    }
}