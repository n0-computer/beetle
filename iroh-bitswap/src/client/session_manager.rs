use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use ahash::AHashMap;
use anyhow::{anyhow, bail, Result};
use cid::Cid;
use futures::FutureExt;
use iroh_metrics::{bitswap::BitswapMetrics, core::MRecorder, inc};
use libp2p::PeerId;
use tokio::sync::RwLock;
use tracing::debug;

use crate::{network::Network, Block};

use super::{
    block_presence_manager::BlockPresenceManager,
    peer_manager::PeerManager,
    session::{BlockReceiver, Session},
    session_interest_manager::SessionInterestManager,
};

#[derive(Debug, Clone)]
pub struct SessionManager {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    session_interest_manager: SessionInterestManager,
    block_presence_manager: BlockPresenceManager,
    peer_manager: PeerManager,
    network: Network,
    sessions: RwLock<AHashMap<u64, Session>>,
    session_index: AtomicU64,
    notify: async_broadcast::Sender<Block>,
}

impl SessionManager {
    pub async fn new(
        self_id: PeerId,
        network: Network,
        notify: async_broadcast::Sender<Block>,
    ) -> Self {
        let session_interest_manager = SessionInterestManager::default();
        let block_presence_manager = BlockPresenceManager::new();
        let peer_manager = PeerManager::new(self_id, network.clone()).await;

        let this = SessionManager {
            inner: Arc::new(Inner {
                session_interest_manager,
                block_presence_manager,
                peer_manager,
                network,
                sessions: Default::default(),
                session_index: Default::default(),
                notify,
            }),
        };

        this.inner
            .peer_manager
            .set_cb({
                let this = this.clone();
                move |peer: PeerId, dont_haves: Vec<Cid>| {
                    let this = this.clone();
                    async move {
                        this.receive_from(Some(peer), &[][..], &[][..], &dont_haves)
                            .await;
                    }
                    .boxed()
                }
            })
            .await;
        this
    }

    pub fn peer_manager(&self) -> &PeerManager {
        &self.inner.peer_manager
    }

    pub fn block_presence_manager(&self) -> &BlockPresenceManager {
        &self.inner.block_presence_manager
    }

    pub fn session_interest_manager(&self) -> &SessionInterestManager {
        &self.inner.session_interest_manager
    }

    pub async fn stop(self) -> Result<()> {
        let inner = Arc::try_unwrap(self.inner)
            .map_err(|_| anyhow!("session manager refs not shutdown"))?;

        let sessions = RwLock::into_inner(inner.sessions);
        let results = futures::future::join_all(
            sessions
                .into_iter()
                .map(|(_, session)| async move { session.stop().await }),
        )
        .await;

        for r in results {
            r?;
        }

        inner.peer_manager.stop().await?;

        Ok(())
    }

    /// Initializes a new session and starts tracking it.
    pub async fn new_session(
        &self,
        provider_search_delay: Duration,
        rebroadcast_delay: Duration,
    ) -> u64 {
        let id = self.get_next_session_id().await;
        self.new_session_with_id(id, provider_search_delay, rebroadcast_delay)
            .await
    }

    async fn new_session_with_id(
        &self,
        session_id: u64,
        provider_search_delay: Duration,
        rebroadcast_delay: Duration,
    ) -> u64 {
        inc!(BitswapMetrics::SessionsCreated);

        let session = Session::new(
            session_id,
            self.clone(),
            self.inner.network.clone(),
            self.inner.notify.clone(),
            provider_search_delay,
            rebroadcast_delay,
        )
        .await;

        self.inner
            .sessions
            .write()
            .await
            .insert(session_id, session);
        session_id
    }

    pub async fn get_or_create_session(
        &self,
        session_id: u64,
        provider_search_delay: Duration,
        rebroadcast_delay: Duration,
    ) -> u64 {
        if self.inner.sessions.read().await.contains_key(&session_id) {
            return session_id;
        }

        self.new_session_with_id(session_id, provider_search_delay, rebroadcast_delay)
            .await
    }

    pub async fn get_block(&self, session_id: u64, key: &Cid) -> Result<Block> {
        let recv = self.get_blocks(session_id, &[*key]).await?;
        let block = recv.recv().await?;
        Ok(block)
    }

    pub async fn get_blocks(&self, session_id: u64, keys: &[Cid]) -> Result<BlockReceiver> {
        if let Some(session) = self.inner.sessions.read().await.get(&session_id) {
            let blocks = session.get_blocks(keys).await?;
            Ok(blocks)
        } else {
            bail!("Session {} not found", session_id);
        }
    }

    pub async fn remove_session(&self, session_id: u64) -> Result<()> {
        debug!(
            "stopping session {} ({} sessions)",
            session_id,
            self.inner.sessions.read().await.len()
        );
        if let Some(session) = self.inner.sessions.write().await.remove(&session_id) {
            session.stop().await?;
            let cancels = self
                .inner
                .session_interest_manager
                .remove_session(session_id)
                .await;
            self.cancel_wants(&cancels).await;
        }
        debug!("stopping session {} done", session_id);
        Ok(())
    }

    /// Returns the next sequential identifier for a session.
    pub async fn get_next_session_id(&self) -> u64 {
        // check that we didn't already use the current one
        for _ in 0..1000 {
            let id = self.inner.session_index.fetch_add(1, Ordering::SeqCst);
            if !self.inner.sessions.read().await.contains_key(&id) {
                return id;
            }
        }
        panic!("unable to retrieve a valid session id after 1000 iterations");
    }

    pub async fn receive_from(
        &self,
        peer: Option<PeerId>,
        blocks: &[Cid],
        haves: &[Cid],
        dont_haves: &[Cid],
    ) {
        debug!(
            "received updates from: {:?} keys: {:?}\n  haves: {:?}\n  dont_haves: {:?}",
            peer.map(|s| s.to_string()),
            blocks.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            haves.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            dont_haves.iter().map(|s| s.to_string()).collect::<Vec<_>>()
        );

        // Record block presence for HAVE/DONT_HAVE.
        if let Some(ref peer) = peer {
            self.inner
                .block_presence_manager
                .receive_from(peer, haves, dont_haves)
                .await;
        }

        {
            let sessions = &*self.inner.sessions.read().await;
            for id in &self
                .inner
                .session_interest_manager
                .interested_sessions(blocks, haves, dont_haves)
                .await
            {
                if let Some(session) = sessions.get(id) {
                    // The SessionManager tells each Session about all keys that it may be
                    // interested in. Here the Session filters the keys to the ones that this
                    // particular Session is interested in.
                    let interested_keys = self
                        .inner
                        .session_interest_manager
                        .filter_session_interested(*id, blocks)
                        .await;

                    let interested_haves = self
                        .inner
                        .session_interest_manager
                        .filter_session_interested(*id, haves)
                        .await;

                    let interested_dont_haves = self
                        .inner
                        .session_interest_manager
                        .filter_session_interested(*id, dont_haves)
                        .await;

                    session
                        .receive_from(
                            peer,
                            interested_keys,
                            interested_haves,
                            interested_dont_haves,
                        )
                        .await;
                }
            }
        }

        // Send CANCELs to all peers with want-have/want-block
        self.inner.peer_manager.send_cancels(blocks).await;
    }

    pub async fn cancel_session_wants(&self, session_id: u64, wants: &[Cid]) {
        // Remove session's interest in the given blocks - returns the keys taht
        // no session is interested in anymore.
        let cancels = self
            .inner
            .session_interest_manager
            .remove_session_interested(session_id, wants)
            .await;
        self.cancel_wants(&cancels).await;
    }

    async fn cancel_wants(&self, wants: &[Cid]) {
        // Free up block presence tracking
        self.inner.block_presence_manager.remove_keys(wants).await;

        // Send CANCEL to all peers for blocks that no session is interested anymore.
        self.inner.peer_manager.send_cancels(wants).await;
    }
}
