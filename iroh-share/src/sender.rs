use std::path::Path;

use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use futures::channel::oneshot::{channel as oneshot, Receiver as OneShotReceiver};
use futures::StreamExt;
use iroh_rpc_types::GossipsubEvent;
use iroh_unixfs::builder::{DirectoryBuilder, FileBuilder};
use libp2p::gossipsub::Sha256Topic;
use rand::Rng;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::{
    p2p_node::{P2pNode, Ticket},
    ReceiverMessage, SenderMessage,
};

/// The sending part of the data transfer.
#[derive(Debug)]
pub struct Sender {
    p2p: P2pNode,
}

impl Sender {
    pub async fn new(port: u16, db_path: &Path) -> Result<Self> {
        let p2p = P2pNode::new(port, db_path).await?;

        Ok(Sender { p2p })
    }

    pub async fn transfer_from_dir_builder(
        self,
        dir_builder: DirectoryBuilder,
    ) -> Result<Transfer> {
        let id = self.next_id();
        let Sender { p2p } = self;

        let t = Sha256Topic::new(format!("iroh-share-{id}"));
        let root_dir = dir_builder.build().await?;

        let (done_sender, done_receiver) = oneshot();

        let p2p_rpc = p2p.rpc().try_p2p()?;
        let store = p2p.rpc().try_store()?;
        let (root, num_parts) = {
            let parts = root_dir.encode();
            tokio::pin!(parts);
            let mut num_parts = 0;
            let mut root_cid = None;
            while let Some(part) = parts.next().await {
                let (cid, bytes, links) = part?.into_parts();
                num_parts += 1;
                root_cid = Some(cid);
                store.put(cid, bytes, links).await?;
            }
            (root_cid.unwrap(), num_parts)
        };

        let topic_hash = t.hash();
        let th = topic_hash.clone();

        // subscribe to the topic, to receive responses
        let mut subscription = p2p_rpc.gossipsub_subscribe(topic_hash.clone()).await?;
        let p2p2 = p2p_rpc.clone();
        let gossip_task_source = tokio::task::spawn(async move {
            let mut current_peer = None;
            while let Some(Ok(event)) = subscription.next().await {
                match event {
                    GossipsubEvent::Subscribed { peer_id, topic } => {
                        if topic == th && current_peer.is_none() {
                            info!("connected to {}", peer_id);
                            current_peer = Some(peer_id);

                            let start =
                                bincode::serialize(&SenderMessage::Start { root, num_parts })
                                    .expect("serialize failure");
                            p2p2.gossipsub_publish(topic.clone(), start.into())
                                .await
                                .unwrap();
                        }
                    }
                    GossipsubEvent::Message { from, message, .. } => {
                        debug!("received message from {}", from);
                        if let Some(current_peer) = current_peer {
                            if from == current_peer {
                                match bincode::deserialize(&message.data) {
                                    Ok(ReceiverMessage::FinishOk) => {
                                        info!("finished transfer");
                                        done_sender.send(Ok(())).ok();
                                        break;
                                    }
                                    Ok(ReceiverMessage::FinishError(err)) => {
                                        info!("transfer failed: {}", err);
                                        done_sender.send(Err(anyhow!("{}", err))).ok();
                                        break;
                                    }
                                    Err(err) => {
                                        warn!("unexpected message: {:?}", err);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        });

        let (peer_id, addrs) = p2p_rpc
            .get_listening_addrs()
            .await
            .context("getting p2p info")?;
        info!("Available addrs: {:?}", addrs);
        let topic_string = topic_hash.to_string();

        let ticket = Ticket {
            peer_id,
            addrs,
            topic: topic_string,
        };

        Ok(Transfer {
            ticket,
            gossip_task_source,
            done_receiver,
            p2p,
        })
    }

    pub async fn transfer_from_data(
        self,
        name: impl Into<String>,
        data: Bytes,
    ) -> Result<Transfer> {
        let name = name.into();
        // wrap in directory to preserve the name
        let file = FileBuilder::new()
            .name(name)
            .content_bytes(data)
            .build()
            .await?;
        let root_dir = DirectoryBuilder::new().add_file(file);

        self.transfer_from_dir_builder(root_dir).await
    }

    fn next_id(&self) -> u64 {
        rand::thread_rng().gen()
    }
}

#[derive(Debug)]
pub struct Transfer {
    p2p: P2pNode,
    ticket: Ticket,
    done_receiver: OneShotReceiver<Result<()>>,
    gossip_task_source: JoinHandle<()>,
}

impl Transfer {
    pub fn ticket(&self) -> &Ticket {
        &self.ticket
    }

    /// Waits until the transfer is done.
    pub async fn done(self) -> Result<()> {
        self.done_receiver.await??;
        self.gossip_task_source.await?;
        self.p2p.close().await?;

        Ok(())
    }
}
