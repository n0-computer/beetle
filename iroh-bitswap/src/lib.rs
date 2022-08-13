//! Implementation of [bitswap](https://github.com/ipfs/specs/blob/master/BITSWAP.md).

// mod error;
// mod message;
// mod prefix;

// Notes
// - HashMap & HashSets need all to be limited in size, so need wrappers
//   to avoid arbitrary growth
// - ConnectionHandlers should keep connections open as long as they are part
//   of a session
// -

use std::time::{Duration, Instant};

use block::Block;
use cid::Cid;
use collections::{BoundedHashMap, BoundedHashSet};
use libp2p::{Multiaddr, PeerId};

mod block;
mod collections;

pub struct Bitswap {
    address_book: AddressBook,
    peers: Peers,
    sessions: Sessions,
    config: Config,
}

type RequestId = usize;
type Priority = u32;

struct Config {
    limits: Limits,
    dial_timeout: Duration,
    keep_alive_timeout: Duration,
    /// How many peers to track at most.
    max_peers: usize,
}

struct Limits {
    max_sessions: usize,
    session_limits: SessionLimits,
}

struct SessionLimits {
    max_peers: usize,
    max_requests: usize,
}

/// Stores seen addresses about a given peer.
struct AddressBook {
    peers: BoundedHashMap<PeerId, BoundedHashSet<Multiaddr>>,
}

pub struct Sessions {
    sessions: BoundedHashMap<PeerId, Session>,
}

pub struct Session {
    peers: BoundedHashMap<PeerId, SessionPeerState>,
    requests: BoundedHashMap<RequestId, Request>,
}

/// Stores peers and status information about them
/// Peers will be removed after some time that they are not used in any session anymore.
pub struct Peers {
    peers: BoundedHashMap<PeerId, Peer>,
}

struct Peer {
    state: PeerState,
    stats: PeerStats,

    ledger: Ledger,
}

struct Ledger {
    /// the wants this peer as has sent to us
    received_want_blocks: BoundedHashSet<Cid>,
    /// the wanthaves this peer has sent to us
    received_want_haves: BoundedHashSet<Cid>,
}

/// Peer state specific to a session
struct SessionPeerState {
    /// List of cids we have reason to believe this peer provides.
    /// - DHT provides and Have messages add entries to this list
    /// - DontHave messages remove entries from this list
    provides_blocks: BoundedHashSet<Provide>,

    /// Tracks the outstanding wants againts this peers (from us)
    sent_wantlist: Wantlist,

    /// the blocks this peer has sent to us
    received_blocks: BoundedHashSet<Cid>,
}

enum Provide {
    /// DHT informed us about this provide
    Dht(Cid),
    /// The peer sent us a Have for this provide.
    Bitswap(Cid),
}

struct Wantlist {
    /// When did we send the last wantlist update. None if we haven't sent it yet
    last_sent: Option<Instant>,
    /// the wants we have sent to this peer
    want_blocks: BoundedHashMap<Cid, Priority>,
    /// the want haves we have sent to this peer
    want_haves: BoundedHashSet<Cid>,
}

struct PeerStats {
    /// Current latency as reported by ping
    latency: Duration,
    /// the load of the peer based on `pendingBytes`
    load: usize,
}

enum PeerState {
    /// Newly added no confirmation,
    New,
    /// Currently disconnected, but still useful potentially.
    Disconnected { last_connected: Instant },
    /// Currently dialing.
    Dialing { dial_start: Instant },
    /// Currently connected and ready to send.
    Connected,
}

pub struct Request {
    typ: RequestType,
    receivers: Vec<PeerId>,
}

pub struct Response {
    sender: PeerId,
    typ: ResponseType,
}

pub enum RequestType {
    /// Send this block once you get it.
    WantBlock { cid: Cid },
    /// Tell me when you have this block.
    WantHave { cid: Cid },
    /// I am no longer interested in Want & WantHaves of this block.
    Cancel { cid: Cid },
    /// Tell me if you have this block.
    DontHave { cid: Cid },
}

pub enum ResponseType {
    /// I do not have this block. (response to DontHave)
    DontHave { cid: Cid },
    /// I have this block. (response to WantHave)
    Have { cid: Cid },
    /// Here is the block. (response to WantBlock)
    Block { block: Block },
}

// // fetching content from the resolvers view
// fn get(cid: Cid) -> Result<Block> {
//     // The session should be reused across a full resolve call.
//     let session = Session::new();
//     let pending = session.get(cid);

//     task::spawn({
//         // As new providers are discovered from the DHT, inform the session
//         let providers = node.kad_get_providers(cid);
//         while let Some(provider) = providers.next() {
//             session.inject_provider(cid, provider);
//         }
//     });

//     // await the get to finish
//     let block = pending.await?;
//     Ok(block)
// }

pub struct PendingGet {}

impl Session {
    pub fn new() -> Self {
        todo!()
    }

    pub fn get(&self, cid: Cid) -> PendingGet {
        todo!()
    }

    pub fn inject_provider(&self, cid: Cid, provider: PeerId) {
        todo!()
    }

    pub fn inject_addrs_for_provider(&self, provider: PeerId, addrs: &[Multiaddr]) {
        todo!()
    }
}
