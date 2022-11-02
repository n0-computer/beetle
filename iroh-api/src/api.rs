use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::api_ext::{get_root_path, save_get_stream};
use crate::config::{Config, CONFIG_FILE_NAME, ENV_PREFIX};
#[cfg(feature = "testing")]
use crate::p2p::MockP2p as P2p;
#[cfg(not(feature = "testing"))]
use crate::p2p::P2p;
use crate::{AddEvent, IpfsPath};
use anyhow::{anyhow, Context as _, Result};
use cid::Cid;
use futures::future::{BoxFuture, LocalBoxFuture};
use futures::stream::LocalBoxStream;
use futures::FutureExt;
use futures::{StreamExt, TryStreamExt};
use iroh_resolver::unixfs_builder;
use iroh_rpc_client::Client;
use iroh_rpc_client::StatusTable;
use iroh_util::{iroh_config_path, make_config};
#[cfg(feature = "testing")]
use mockall::automock;
#[cfg(feature = "testing")]
use mockall_double::double;
use relative_path::RelativePathBuf;
use tokio::io::{AsyncRead, AsyncReadExt};

pub struct Api {
    client: Client,
}

pub enum OutType {
    Dir,
    Reader(Box<dyn AsyncRead + Unpin>),
    Symlink(PathBuf),
}

#[cfg_attr(feature = "testing", automock)]
impl Api {
    pub async fn new<'a>(
        config_path: Option<&'a Path>,
        overrides_map: HashMap<String, String>,
    ) -> Result<Self> {
        let cfg_path = iroh_config_path(CONFIG_FILE_NAME)?;
        let sources = vec![Some(cfg_path), config_path.map(PathBuf::from)];
        let config = make_config(
            // default
            Config::default(),
            // potential config files
            sources,
            // env var prefix for this config
            ENV_PREFIX,
            // map of present command line arguments
            overrides_map,
        )
        .unwrap();

        let client = Client::new(config.rpc_client).await?;

        Ok(Api::from_client(client))
    }

    fn from_client(client: Client) -> Self {
        Self { client }
    }

    pub fn provide(&self, cid: Cid) -> LocalBoxFuture<'_, Result<()>> {
        async move { self.client.try_p2p()?.start_providing(&cid).await }.boxed_local()
    }

    pub fn p2p(&self) -> Result<P2p> {
        let p2p_client = self.client.try_p2p()?;
        Ok(P2p::new(p2p_client))
    }

    pub fn get_stream(
        &self,
        ipfs_path: &IpfsPath,
    ) -> LocalBoxStream<'_, Result<(RelativePathBuf, OutType)>> {
        tracing::debug!("get {:?}", ipfs_path);
        let resolver = iroh_resolver::resolver::Resolver::new(self.client.clone());
        let results = resolver.resolve_recursive_with_paths(ipfs_path.clone());
        let sub_path = ipfs_path.to_relative_string();
        async_stream::try_stream! {
            tokio::pin!(results);
            while let Some(res) = results.next().await {
                let (relative_ipfs_path, out) = res?;
                let relative_path = RelativePathBuf::from_path(&relative_ipfs_path.to_relative_string())?;
                // TODO(faassen) this focusing in on sub-paths should really be handled in the resolver:
                // * it can be tested there far more easily than here (where currently it isn't)
                // * it makes sense to have an API "what does this resolve to" in the resolver
                // * the resolver may have opportunities for optimization we don't have
                if !relative_path.starts_with(&sub_path) {
                    continue;
                }
                let relative_path = relative_path.strip_prefix(&sub_path).expect("should be a prefix").to_owned();
                if out.is_dir() {
                    yield (relative_path, OutType::Dir);
                } else if out.is_symlink() {
                    let mut reader = out.pretty(resolver.clone(), Default::default(), iroh_resolver::resolver::ResponseClip::NoClip)?;
                    let mut target = String::new();
                    reader.read_to_string(&mut target).await?;
                    let target = PathBuf::from(target);
                    yield (relative_path, OutType::Symlink(target));
                } else {
                    let reader = out.pretty(resolver.clone(), Default::default(), iroh_resolver::resolver::ResponseClip::NoClip)?;
                    yield (relative_path, OutType::Reader(Box::new(reader)));
                }
            }
        }
        .boxed_local()
    }

    pub fn add_file(
        &self,
        path: &Path,
        wrap: bool,
    ) -> LocalBoxFuture<'_, Result<LocalBoxStream<'static, Result<AddEvent>>>> {
        let providing_client = iroh_resolver::unixfs_builder::StoreAndProvideClient {
            client: self.client.clone(),
        };
        let path = path.to_path_buf();
        async move {
            unixfs_builder::add_file(Some(providing_client), &path, wrap)
                .await
                .map(|s| s.boxed_local())
        }
        .boxed_local()
    }

    pub fn add_dir(
        &self,
        path: &Path,
        wrap: bool,
    ) -> LocalBoxFuture<'_, Result<LocalBoxStream<'static, Result<AddEvent>>>> {
        let providing_client = iroh_resolver::unixfs_builder::StoreAndProvideClient {
            client: self.client.clone(),
        };
        let path = path.to_path_buf();
        async move {
            unixfs_builder::add_dir(Some(providing_client), &path, wrap)
                .await
                .map(|s| s.boxed_local())
        }
        .boxed_local()
    }

    pub fn add_symlink(
        &self,
        path: &Path,
        wrap: bool,
    ) -> LocalBoxFuture<'_, Result<LocalBoxStream<'static, Result<AddEvent>>>> {
        let providing_client = iroh_resolver::unixfs_builder::StoreAndProvideClient {
            client: self.client.clone(),
        };
        let path = path.to_path_buf();
        async move {
            unixfs_builder::add_symlink(Some(providing_client), &path, wrap)
                .await
                .map(|s| s.boxed_local())
        }
        .boxed_local()
    }

    pub fn check(&self) -> BoxFuture<'_, StatusTable> {
        async { self.client.check().await }.boxed()
    }

    pub fn watch(
        &self,
    ) -> LocalBoxFuture<'static, LocalBoxStream<'static, iroh_rpc_client::StatusTable>> {
        let client = self.client.clone();
        async { client.watch().await.boxed_local() }.boxed_local()
    }

    /// High level get, equivalent of CLI `iroh get`
    pub async fn get<'a>(
        &self,
        ipfs_path: &IpfsPath,
        output_path: Option<&'a Path>,
    ) -> Result<PathBuf> {
        if ipfs_path.cid().is_none() {
            return Err(anyhow!("IPFS path does not refer to a CID"));
        }
        let root_path = get_root_path(ipfs_path, output_path);
        if root_path.exists() {
            return Err(anyhow!(
                "output path {} already exists",
                root_path.display()
            ));
        }
        let blocks = self.get_stream(ipfs_path);
        save_get_stream(&root_path, blocks).await?;
        Ok(root_path)
    }

    pub async fn add_stream(
        &self,
        path: &Path,
        wrap: bool,
    ) -> Result<LocalBoxStream<'static, Result<AddEvent>>> {
        if path.is_dir() {
            self.add_dir(path, wrap).await
        } else if path.is_symlink() {
            self.add_symlink(path, wrap).await
        } else if path.is_file() {
            self.add_file(path, wrap).await
        } else {
            anyhow::bail!("can only add files or directories")
        }
    }

    pub async fn add(&self, path: &Path, wrap: bool) -> Result<Cid> {
        let add_events = self.add_stream(path, wrap).await?;

        add_events
            .try_fold(None, |_acc, add_event| async move {
                match add_event {
                    AddEvent::ProgressDelta { cid, .. } => Ok(Some(cid)),
                }
            })
            .await?
            .context("No cid found")
    }
}
