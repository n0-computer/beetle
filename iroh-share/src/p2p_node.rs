use std::{collections::HashSet, path::Path, sync::Arc};

use anyhow::{ensure, Result};
use async_trait::async_trait;
use cid::Cid;
use iroh_p2p::{config, Config, Keychain, MemoryStorage, Node};
use iroh_resolver::resolver::Resolver;
use iroh_rpc_client::Client;
use iroh_rpc_types::Addr;
use iroh_unixfs::{
    content_loader::{ContentLoader, ContextId, LoaderContext, IROH_STORE},
    parse_links, LoadedCid, Source,
};
use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Serialize};
use tokio::{sync::Mutex, task::JoinHandle};
use tracing::{error, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ticket {
    pub peer_id: PeerId,
    pub addrs: Vec<Multiaddr>,
    pub topic: String,
}

impl Ticket {
    pub fn as_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).expect("failed to serialize")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let ticket = bincode::deserialize(bytes)?;
        Ok(ticket)
    }
}

#[derive(Debug)]
pub struct P2pNode {
    p2p_task: JoinHandle<()>,
    store_task: JoinHandle<()>,
    rpc: Client,
    resolver: Resolver<Loader>,
}

/// Wrapper struct to implement custom content loading
#[derive(Debug, Clone)]
pub struct Loader {
    client: Client,
    providers: Arc<Mutex<HashSet<PeerId>>>,
}

impl Loader {
    pub fn new(client: Client) -> Self {
        Loader {
            client,
            providers: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn providers(&self) -> &Arc<Mutex<HashSet<PeerId>>> {
        &self.providers
    }
}

#[async_trait]
impl ContentLoader for Loader {
    async fn load_cid(&self, cid: &Cid, _ctx: &LoaderContext) -> Result<LoadedCid> {
        let cid = *cid;
        let providers = self.providers.lock().await.clone();

        match self.client.try_store()?.get(cid).await {
            Ok(Some(data)) => {
                return Ok(LoadedCid {
                    data,
                    source: Source::Store(IROH_STORE),
                });
            }
            Ok(None) => {}
            Err(err) => {
                warn!("failed to fetch data from store {}: {:?}", cid, err);
            }
        }

        ensure!(!providers.is_empty(), "no providers supplied");

        // TODO: track context id
        let res = self
            .client
            .try_p2p()?
            .fetch_bitswap(0, cid, providers.clone())
            .await;
        let bytes = match res {
            Ok(bytes) => bytes,
            Err(err) => {
                error!("Bitswap error: {:#?}", err);
                return Err(err);
            }
        };

        let cloned = bytes.clone();
        let rpc = self.clone();
        {
            let clone2 = cloned.clone();
            let links =
                tokio::task::spawn_blocking(move || parse_links(&cid, &clone2).unwrap_or_default())
                    .await
                    .unwrap_or_default();

            rpc.client.try_store()?.put(cid, cloned, links).await?;
        }

        Ok(LoadedCid {
            data: bytes,
            source: Source::Bitswap,
        })
    }

    async fn stop_session(&self, ctx: ContextId) -> Result<()> {
        self.client
            .try_p2p()?
            .stop_session_bitswap(ctx.into())
            .await?;
        Ok(())
    }

    async fn has_cid(&self, cid: &Cid) -> Result<bool> {
        Ok(self.client.try_store()?.has(*cid).await?)
    }
}

impl P2pNode {
    pub async fn new(port: u16, db_path: &Path) -> Result<Self> {
        let rpc_p2p_addr_server = Addr::new_mem();
        let rpc_p2p_addr_client = rpc_p2p_addr_server.clone();
        let rpc_store_addr_server = Addr::new_mem();
        let rpc_store_addr_client = rpc_store_addr_server.clone();

        let rpc_store_client_config = iroh_rpc_client::Config {
            p2p_addr: Some(rpc_p2p_addr_client.clone()),
            store_addr: Some(rpc_store_addr_client.clone()),
            gateway_addr: None,
            channels: Some(1),
        };
        let rpc_p2p_client_config = iroh_rpc_client::Config {
            p2p_addr: Some(rpc_p2p_addr_client.clone()),
            store_addr: Some(rpc_store_addr_client.clone()),
            gateway_addr: None,
            channels: Some(1),
        };
        let mut libp2p_config = config::Libp2pConfig::default();
        libp2p_config.listening_multiaddrs =
            vec![format!("/ip4/0.0.0.0/tcp/{port}").parse().unwrap()];
        libp2p_config.mdns = false;
        libp2p_config.kademlia = true;
        libp2p_config.autonat = true;
        libp2p_config.relay_client = true;
        libp2p_config.bootstrap_peers = Default::default(); // disable bootstrap for now
        libp2p_config.relay_server = false;
        libp2p_config.max_conns_in = 8;
        libp2p_config.max_conns_out = 8;
        let config = Config {
            libp2p: libp2p_config,
            rpc_client: rpc_p2p_client_config.clone(),
            key_store_path: db_path.parent().unwrap().to_path_buf(),
        };

        let rpc = Client::new(rpc_p2p_client_config).await?;
        let loader = Loader::new(rpc.clone());
        let resolver = iroh_resolver::resolver::Resolver::new(loader);

        let store_config = iroh_store::Config {
            path: db_path.to_path_buf(),
            rpc_client: rpc_store_client_config,
        };

        let store = if store_config.path.exists() {
            iroh_store::Store::open(store_config).await?
        } else {
            iroh_store::Store::create(store_config).await?
        };

        let kc = Keychain::<MemoryStorage>::new();
        let mut p2p = Node::new(config, rpc_p2p_addr_server, kc).await?;

        let p2p_task = tokio::task::spawn(async move {
            if let Err(err) = p2p.run().await {
                error!("{:?}", err);
            }
        });

        let store_task = tokio::spawn(async move {
            iroh_store::rpc::new(rpc_store_addr_server, store)
                .await
                .unwrap()
        });

        Ok(Self {
            p2p_task,
            store_task,
            rpc,
            resolver,
        })
    }

    pub fn rpc(&self) -> &Client {
        &self.rpc
    }

    pub fn resolver(&self) -> &Resolver<Loader> {
        &self.resolver
    }

    pub async fn close(self) -> Result<()> {
        self.rpc.try_p2p().unwrap().shutdown().await?;
        self.store_task.abort();
        self.p2p_task.await?;
        self.store_task.await.ok();
        Ok(())
    }
}
