use ahash::{AHashMap, AHashSet};
use bytes::Bytes;
use cid::Cid;
use libp2p::{
    swarm::{NetworkBehaviourAction, NotifyHandler},
    Multiaddr, PeerId,
};
use tracing::{debug, error, trace};

use crate::{
    behaviour::{BitswapHandler, Provider, QueryError},
    BitswapEvent, BitswapMessage, Block, CancelResult, Priority, QueryResult, SendResult,
    WantResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QueryId(usize);

#[derive(Default, Debug)]
pub struct QueryManager {
    queries: AHashMap<QueryId, Query>,
    next_id: usize,
}

impl QueryManager {
    fn new_query(&mut self, query: Query) -> QueryId {
        let id = QueryId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.queries.insert(id, query);
        id
    }

    pub fn is_empty(&self) -> bool {
        self.queries.is_empty()
    }

    fn cancel_len(&self) -> usize {
        self.queries
            .values()
            .filter(|q| matches!(q, Query::Cancel { .. }))
            .count()
    }

    fn want_len(&self) -> usize {
        self.queries
            .values()
            .filter(|q| matches!(q, Query::Want { .. }))
            .count()
    }

    fn send_len(&self) -> usize {
        self.queries
            .values()
            .filter(|q| matches!(q, Query::Send { .. }))
            .count()
    }

    pub fn want(&mut self, cid: Cid, priority: Priority) -> QueryId {
        self.new_query(Query::Want {
            cid,
            priority,
            state: State::New,
        })
    }

    pub fn send(
        &mut self,
        receiver: PeerId,
        addrs: &[Multiaddr],
        cid: Cid,
        data: Bytes,
    ) -> QueryId {
        self.new_query(Query::Send {
            receiver: (receiver, addrs.into()),
            block: Block { cid, data },
            state: State::New,
        })
    }

    pub fn cancel(&mut self, cid: &Cid) -> Option<QueryId> {
        let mut cancel = None;
        self.queries.retain(|_, query| match query {
            Query::Want {
                cid: c,
                priority: _,
                state,
            } => {
                let to_remove = cid == c;
                if to_remove {
                    if let State::Sent(providers) = state {
                        // send out cancels to the providers
                        cancel = Some((providers.clone(), *cid));
                    }
                }
                !to_remove
            }
            Query::Cancel { .. } => true,
            Query::Send { .. } => true,
        });

        cancel.map(|(receivers, cid)| {
            self.new_query(Query::Cancel {
                cid,
                state: State::New,
                receivers,
            })
        })
    }

    pub fn process_block(&mut self, sender: &PeerId, block: &Block) -> Vec<QueryId> {
        tracing::debug!("process block from {}", sender);
        let mut cancels = Vec::new();
        let mut query_ids = Vec::new();

        self.queries.retain(|id, query| {
            match query {
                Query::Want {
                    cid,
                    priority: _,
                    state,
                } => {
                    if &block.cid == cid {
                        query_ids.push(*id);

                        if let State::Sent(receivers) = state {
                            // send out cancels to the providers
                            let mut receivers = receivers.clone();
                            receivers.remove(sender);
                            if !receivers.is_empty() {
                                cancels.push((receivers, block.cid));
                            }
                        }
                        false
                    } else {
                        true
                    }
                }
                Query::Cancel { .. } => true,
                Query::Send { .. } => true,
            }
        });

        for (receivers, cid) in cancels.into_iter() {
            self.new_query(Query::Cancel {
                cid,
                state: State::New,
                receivers,
            });
        }

        query_ids
    }

    /// Handle disconnection of the endpoint
    pub fn disconnected(&mut self, peer_id: &PeerId) {
        // nothing to do for the moment
        tracing::debug!("disconnected from {}", peer_id);
    }

    pub fn dial_failure(&mut self, peer_id: &PeerId) {
        tracing::debug!("disconnected from {}", peer_id);
        for (_, query) in self
            .queries
            .iter_mut()
            .filter(|(_, query)| query.contains_provider(peer_id))
        {
            debug!("disconnected {}, {:?}", peer_id, query);
            match query {
                Query::Want { state, .. } => {
                    if let State::Sent(used_providers) = state {
                        used_providers.remove(peer_id);
                    }
                }
                Query::Send { state, .. } => {
                    if let State::Sent(used_providers) = state {
                        used_providers.remove(peer_id);
                    }
                }
                Query::Cancel { state, .. } => {
                    if let State::Sent(used_providers) = state {
                        used_providers.remove(peer_id);
                    }
                }
            }
        }
    }

    fn next_finished_query(
        &mut self,
        providers: &AHashMap<Cid, Vec<Provider>>,
    ) -> Option<(QueryId, Query)> {
        let next_query = self.queries.iter().find(|(query_id, query)| match query {
            Query::Want { cid, state, .. } => {
                let has_providers = providers
                    .get(&cid)
                    .map(|v| !v.is_empty())
                    .unwrap_or_default();

                match state {
                    State::New => !has_providers,
                    State::Sent(used_providers) => used_providers.is_empty() && !has_providers,
                }
            }
            Query::Send { state, .. } => match state {
                State::New => false,
                State::Sent(used_providers) => used_providers.is_empty(),
            },
            Query::Cancel { state, .. } => match state {
                State::New => false,
                State::Sent(used_providers) => used_providers.is_empty(),
            },
        });

        if let Some((id, _)) = next_query {
            let id = *id;
            return Some((id, self.queries.remove(&id).unwrap()));
        }

        None
    }

    pub(crate) fn poll_all(
        &mut self,
        providers: &AHashMap<Cid, Vec<Provider>>,
    ) -> Option<NetworkBehaviourAction<BitswapEvent, BitswapHandler>> {
        self.next_finished_query(providers)
            .map(|(id, query)| match query {
                Query::Send { .. } => (id, QueryResult::Send(SendResult::Err(QueryError::Timeout))),
                Query::Want { .. } => (id, QueryResult::Want(WantResult::Err(QueryError::Timeout))),
                Query::Cancel { .. } => (
                    id,
                    QueryResult::Cancel(CancelResult::Err(QueryError::Timeout)),
                ),
            })
            .map(|(id, result)| {
                NetworkBehaviourAction::GenerateEvent(BitswapEvent::OutboundQueryCompleted {
                    id,
                    result,
                })
            })
    }

    pub(crate) fn poll_peer(
        &mut self,
        peer_id: &PeerId,
        providers: &AHashMap<Cid, Vec<Provider>>,
    ) -> Option<NetworkBehaviourAction<BitswapEvent, BitswapHandler>> {
        if self.is_empty() {
            return None;
        }

        // Aggregate all queries for this peer
        let mut msg = BitswapMessage::default();

        /*trace!(
            "connected {}, looking for queries: {}want, {}cancel, {}send",
            peer_id,
            self.want_len(),
            self.cancel_len(),
            self.send_len()
        );*/
        let mut finished_queries = Vec::new();

        for (query_id, query) in self.queries.iter_mut() {
            match query {
                Query::Want {
                    cid,
                    priority,
                    state,
                } => {
                    if let Some(providers) = providers.get(&cid) {
                        if providers.iter().find(|p| &p.id == peer_id).is_some() {
                            match state {
                                State::New => {
                                    msg.wantlist_mut().want_block(cid, *priority);

                                    *state = State::Sent([*peer_id].into_iter().collect());
                                }
                                State::Sent(sent_providers) => {
                                    if !sent_providers.contains(peer_id) {
                                        msg.wantlist_mut().want_block(cid, *priority);
                                        sent_providers.insert(*peer_id);
                                    }
                                }
                            }
                        }
                    }
                }
                Query::Cancel {
                    cid,
                    state,
                    receivers,
                } => {
                    if receivers.contains(peer_id) {
                        msg.wantlist_mut().cancel_block(cid);

                        // update state
                        match state {
                            State::New => {
                                *state = State::Sent([*peer_id].into_iter().collect());
                            }
                            State::Sent(sent_providers) => {
                                sent_providers.insert(*peer_id);
                            }
                        }
                    }
                }
                Query::Send {
                    block,
                    state,
                    receiver,
                } => {
                    if peer_id == &receiver.0 {
                        match state {
                            State::New => {
                                msg.add_block(block.clone());
                                *state = State::Sent([receiver.0.clone()].into_iter().collect());
                            }
                            State::Sent(_) => {
                                // nothing to do anymore
                                finished_queries.push(*query_id);
                            }
                        }
                    }
                }
            }
        }

        // remove finished queries
        for id in finished_queries {
            self.queries.remove(&id);
        }

        if msg.is_empty() {
            return None;
        }

        debug!(
            "sending message to {} (blocks: {}, wants: {})",
            peer_id,
            msg.blocks().len(),
            msg.wantlist().blocks().count()
        );
        Some(NetworkBehaviourAction::NotifyHandler {
            peer_id: *peer_id,
            handler: NotifyHandler::Any,
            event: msg,
        })
    }
}

#[derive(Debug)]
enum Query {
    /// Fetch a single CID.
    Want {
        cid: Cid,
        priority: Priority,
        state: State,
    },
    /// Cancel a single CID.
    Cancel {
        cid: Cid,
        state: State,
        receivers: AHashSet<PeerId>,
    },
    /// Sends a single Block.
    Send {
        receiver: (PeerId, Vec<Multiaddr>),
        block: Block,
        state: State,
    },
}

impl Query {
    fn contains_provider(&self, peer_id: &PeerId) -> bool {
        match self {
            Query::Want { state, .. } | Query::Cancel { state, .. } => {
                if let State::Sent(p) = state {
                    return p.contains(peer_id);
                }
                false
            }
            Query::Send {
                receiver, state, ..
            } => {
                if &receiver.0 == peer_id {
                    return true;
                }
                if let State::Sent(p) = state {
                    return p.contains(peer_id);
                }
                false
            }
        }
    }
}

#[derive(Debug)]
enum State {
    New,
    Sent(AHashSet<PeerId>),
}

#[cfg(test)]
mod tests {
    use libp2p::identity::Keypair;

    use super::*;
    use crate::block::tests::create_block;

    #[test]
    fn test_want_success() {
        let mut queries = QueryManager::default();

        let Block { cid, data } = create_block(&b"hello world"[..]);

        let provider_key_1 = Keypair::generate_ed25519();
        let provider_id_1 = provider_key_1.public().to_peer_id();
        let provider_key_2 = Keypair::generate_ed25519();
        let provider_id_2 = provider_key_2.public().to_peer_id();
        let providers = [(
            cid,
            vec![Provider::from(provider_id_1), Provider::from(provider_id_2)],
        )]
        .into_iter()
        .collect();

        assert!(queries.poll_peer(&provider_id_1, &providers).is_none());
        let query_id = queries.want(cid, 100);

        // sent wantlist
        let q = queries.poll_peer(&provider_id_1, &providers).unwrap();
        if let NetworkBehaviourAction::NotifyHandler { peer_id, event, .. } = q {
            assert_eq!(peer_id, provider_id_1);
            assert_eq!(
                event.wantlist().blocks().collect::<Vec<_>>(),
                &[(&cid, 100)]
            );
        } else {
            panic!("invalid poll result");
        }

        // inject received block
        let qid = queries.process_block(&provider_id_1, &Block { cid, data });
        assert_eq!(qid, vec![query_id]);
    }

    #[test]
    fn test_want_fail() {
        let mut queries = QueryManager::default();

        let Block { cid, data: _ } = create_block(&b"hello world"[..]);
        let provider_key_1 = Keypair::generate_ed25519();
        let provider_id_1 = provider_key_1.public().to_peer_id();
        let provider_key_2 = Keypair::generate_ed25519();
        let provider_id_2 = provider_key_2.public().to_peer_id();

        let providers = [(
            cid,
            vec![Provider::from(provider_id_1), Provider::from(provider_id_2)],
        )]
        .into_iter()
        .collect();

        assert!(queries.poll_peer(&provider_id_1, &providers).is_none());

        let query_id = queries.want(cid, 100);

        // send wantlist
        let q = queries.poll_peer(&provider_id_1, &providers).unwrap();
        if let NetworkBehaviourAction::NotifyHandler { peer_id, event, .. } = q {
            assert_eq!(peer_id, provider_id_1);
            assert_eq!(
                event.wantlist().blocks().collect::<Vec<_>>(),
                &[(&cid, 100)]
            );
        } else {
            panic!("invalid poll result");
        }

        let q = queries.poll_peer(&provider_id_2, &providers).unwrap();
        if let NetworkBehaviourAction::NotifyHandler { peer_id, event, .. } = q {
            assert_eq!(peer_id, provider_id_2);
            assert_eq!(
                event.wantlist().blocks().collect::<Vec<_>>(),
                &[(&cid, 100)]
            );
        } else {
            panic!("invalid poll result");
        }

        // inject disconnects
        queries.disconnected(&provider_id_1);
        queries.disconnected(&provider_id_2);

        let q = queries.poll_all(&Default::default()).unwrap();
        if let NetworkBehaviourAction::GenerateEvent(BitswapEvent::OutboundQueryCompleted {
            id,
            ..
        }) = q
        {
            assert_eq!(id, query_id);
        } else {
            panic!("invalid poll result");
        }
    }
}
