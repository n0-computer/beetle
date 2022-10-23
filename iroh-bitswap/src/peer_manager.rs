use std::sync::{Arc, Mutex};
use std::time::Instant;

use ahash::{AHashMap, AHashSet};
use iroh_metrics::{bitswap::BitswapMetrics, core::MRecorder, inc};
use libp2p::{core::connection::ConnectionId, PeerId};

use crate::ProtocolId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerState {
    Connected(AHashSet<ConnectionId>),
    Responsive(AHashSet<ConnectionId>, ProtocolId),
    Disconnected,
    DialFailure(Instant),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerStateChange {
    Connected(ConnectionId),
    Responsive(Option<ConnectionId>, ProtocolId),
    Unresponsive,
    Disconnected(ConnectionId),
    DialFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerAction {
    Connected,
    Disconnected,
}

impl Default for PeerState {
    fn default() -> Self {
        PeerState::Disconnected
    }
}

impl PeerState {
    fn is_connected(&self) -> bool {
        matches!(self, PeerState::Connected(_) | PeerState::Responsive(_, _))
    }
}

#[derive(Debug, Clone, Default)]
pub struct PeerManager {
    peers: Arc<Mutex<AHashMap<PeerId, PeerState>>>,
}

impl PeerManager {
    pub fn get_peer_state(&self, peer: &PeerId) -> Option<PeerState> {
        self.peers.lock().unwrap().get(peer).cloned()
    }

    pub fn set_peer_state(&self, peer: PeerId, new_state: PeerStateChange) -> Option<PeerAction> {
        let peers = &mut *self.peers.lock().unwrap();
        match peers.entry(peer) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let old_state = entry.get_mut();

                match new_state {
                    PeerStateChange::DialFailure | PeerStateChange::Unresponsive => {
                        if old_state.is_connected() {
                            inc!(BitswapMetrics::DisconnectedPeers);
                            return Some(PeerAction::Disconnected);
                        }
                    }
                    PeerStateChange::Disconnected(id) => match old_state {
                        PeerState::Connected(ids) | PeerState::Responsive(ids, _) => {
                            ids.remove(&id);
                            if ids.is_empty() {
                                entry.remove();
                                return Some(PeerAction::Disconnected);
                            }
                        }
                        _ => {
                            // Nothing todo
                        }
                    },
                    PeerStateChange::Connected(id) => match old_state {
                        PeerState::Connected(ids) | PeerState::Responsive(ids, _) => {
                            if ids.insert(id) {
                                inc!(BitswapMetrics::ConnectedPeers);
                            }
                        }
                        PeerState::DialFailure(_) | PeerState::Disconnected => {
                            *old_state = PeerState::Connected([id].into_iter().collect());
                        }
                    },
                    PeerStateChange::Responsive(id, new_protocol) => match old_state {
                        PeerState::Connected(ids) => {
                            inc!(BitswapMetrics::ResponsivePeers);
                            if let Some(id) = id {
                                ids.insert(id);
                            }
                            *old_state = PeerState::Responsive(std::mem::take(ids), new_protocol);
                            return Some(PeerAction::Connected);
                        }
                        PeerState::Disconnected | PeerState::DialFailure(_) => {
                            if let Some(id) = id {
                                inc!(BitswapMetrics::ResponsivePeers);
                                *old_state =
                                    PeerState::Responsive([id].into_iter().collect(), new_protocol);
                                return Some(PeerAction::Connected);
                            }
                        }
                        PeerState::Responsive(ids, old_protocol) => {
                            if let Some(id) = id {
                                ids.insert(id);
                            }
                            *old_protocol = new_protocol;
                        }
                    },
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                match new_state {
                    PeerStateChange::DialFailure => {
                        // record dial failure
                        entry.insert(PeerState::DialFailure(Instant::now()));
                    }
                    PeerStateChange::Unresponsive | PeerStateChange::Disconnected(_) => {
                        // we never knew about this peer, ignore the update
                    }
                    PeerStateChange::Connected(id) => {
                        inc!(BitswapMetrics::ConnectedPeers);
                        entry.insert(PeerState::Connected([id].into_iter().collect()));
                    }
                    PeerStateChange::Responsive(id, protocol) => {
                        if let Some(id) = id {
                            inc!(BitswapMetrics::ResponsivePeers);
                            entry.insert(PeerState::Responsive(
                                [id].into_iter().collect(),
                                protocol,
                            ));
                            return Some(PeerAction::Connected);
                        }
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_connect_disconnect() {
        let pm = PeerManager::default();
        let peers: Vec<PeerId> = (0..10).map(|_| PeerId::random()).collect();

        // Ensure no state is known
        for i in 0..10 {
            assert_eq!(pm.get_peer_state(&peers[i]), None);
        }

        // 5 new connections
        for i in 0..5 {
            assert_eq!(
                pm.set_peer_state(peers[i], PeerStateChange::Connected(ConnectionId::new(i))),
                None,
            );
        }
        // disconnect 3
        for i in 0..3 {
            assert_eq!(
                pm.set_peer_state(
                    peers[i],
                    PeerStateChange::Disconnected(ConnectionId::new(i))
                ),
                Some(PeerAction::Disconnected)
            );
        }

        // identify 2
        for i in 3..5 {
            assert_eq!(
                pm.set_peer_state(
                    peers[i],
                    PeerStateChange::Responsive(Some(ConnectionId::new(i)), ProtocolId::Bitswap120)
                ),
                Some(PeerAction::Connected)
            );
        }

        // reconnect one
        assert_eq!(
            pm.set_peer_state(
                peers[1],
                PeerStateChange::Responsive(Some(ConnectionId::new(1)), ProtocolId::Bitswap120)
            ),
            Some(PeerAction::Connected)
        );

        // check status
        assert_eq!(pm.get_peer_state(&peers[0]), None); // Disconnected
        assert_eq!(
            pm.get_peer_state(&peers[1]),
            Some(PeerState::Responsive(
                [ConnectionId::new(1)].into_iter().collect(),
                ProtocolId::Bitswap120
            ))
        );
        assert_eq!(pm.get_peer_state(&peers[2]), None); // Disconnected
        for i in 3..5 {
            assert_eq!(
                pm.get_peer_state(&peers[i]),
                Some(PeerState::Responsive(
                    [ConnectionId::new(i)].into_iter().collect(),
                    ProtocolId::Bitswap120
                ))
            );
        }
        for i in 5..10 {
            assert_eq!(pm.get_peer_state(&peers[i]), None); // Disconnected
        }
    }

    #[test]
    fn test_multiple_connections() {
        let pm = PeerManager::default();
        let peers: Vec<PeerId> = (0..2).map(|_| PeerId::random()).collect();

        // Ensure no state is known
        for i in 0..2 {
            assert_eq!(pm.get_peer_state(&peers[i]), None);
        }

        // 3 new connections for peer0
        for i in 0..3 {
            assert_eq!(
                pm.set_peer_state(
                    peers[0],
                    PeerStateChange::Connected(ConnectionId::new(i * 100))
                ),
                None,
            );
        }
        assert_eq!(
            pm.get_peer_state(&peers[0]),
            Some(PeerState::Connected(
                [
                    ConnectionId::new(0),
                    ConnectionId::new(100),
                    ConnectionId::new(200)
                ]
                .into_iter()
                .collect(),
            ))
        );

        // Insert double
        assert_eq!(
            pm.set_peer_state(peers[0], PeerStateChange::Connected(ConnectionId::new(0))),
            None,
        );

        assert_eq!(
            pm.get_peer_state(&peers[0]),
            Some(PeerState::Connected(
                [
                    ConnectionId::new(0),
                    ConnectionId::new(100),
                    ConnectionId::new(200)
                ]
                .into_iter()
                .collect(),
            ))
        );

        // Responsive, moves all ids, including the new one
        assert_eq!(
            pm.set_peer_state(
                peers[0],
                PeerStateChange::Responsive(Some(ConnectionId::new(400)), ProtocolId::Bitswap120)
            ),
            Some(PeerAction::Connected),
        );
        assert_eq!(
            pm.get_peer_state(&peers[0]),
            Some(PeerState::Responsive(
                [
                    ConnectionId::new(0),
                    ConnectionId::new(100),
                    ConnectionId::new(200),
                    ConnectionId::new(400)
                ]
                .into_iter()
                .collect(),
                ProtocolId::Bitswap120
            ))
        );

        // disconnecting one, doesn't emit a disconnect
        assert_eq!(
            pm.set_peer_state(
                peers[0],
                PeerStateChange::Disconnected(ConnectionId::new(0))
            ),
            None,
        );

        assert_eq!(
            pm.get_peer_state(&peers[0]),
            Some(PeerState::Responsive(
                [
                    ConnectionId::new(100),
                    ConnectionId::new(200),
                    ConnectionId::new(400)
                ]
                .into_iter()
                .collect(),
                ProtocolId::Bitswap120
            ))
        );
    }
}
