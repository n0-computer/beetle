use anyhow::Result;
use axum::http::header::*;
use config::{ConfigError, Map, Source, Value};

use beetle_metrics::config::Config as MetricsConfig;
use beetle_p2p::Libp2pConfig;
use beetle_rpc_client::Config as RpcClientConfig;
use beetle_store::config::config_data_path;
use beetle_util::insert_into_config_map;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// CONFIG_FILE_NAME is the name of the optional config file located in the beetle home directory
pub const CONFIG_FILE_NAME: &str = "one.config.toml";
/// ENV_PREFIX should be used along side the config field name to set a config field using
/// environment variables
/// For example, `BEETLE_ONE_PORT=1000` would set the value of the `Config.port` field
pub const ENV_PREFIX: &str = "BEETLE_ONE";
pub const DEFAULT_PORT: u16 = 9050;

/// Configuration for [`beetle-one`].
///
/// The configuration includes gateway, store and p2p specific items as well as the common
/// rpc & metrics ones.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Config {
    /// Path for the UDS socket for the gateway.
    #[cfg(all(feature = "http-uds-gateway", unix))]
    pub gateway_uds_path: Option<PathBuf>,
    /// Gateway specific configuration.
    pub gateway: beetle_gateway::config::Config,
    /// Store specific configuration.
    pub store: beetle_store::config::Config,
    /// P2P specific configuration.
    pub p2p: beetle_p2p::config::Config,
    /// rpc addresses for the gateway & addresses for the rpc client to dial
    pub rpc_client: RpcClientConfig,
    /// metrics configuration
    pub metrics: MetricsConfig,
}

impl Config {
    pub fn new(
        gateway: beetle_gateway::config::Config,
        store: beetle_store::config::Config,
        p2p: beetle_p2p::config::Config,
        rpc_client: RpcClientConfig,
        #[cfg(all(feature = "http-uds-gateway", unix))] gateway_uds_path: Option<PathBuf>,
    ) -> Self {
        Self {
            gateway,
            store,
            p2p,
            rpc_client,
            metrics: MetricsConfig::default(),
            #[cfg(all(feature = "http-uds-gateway", unix))]
            gateway_uds_path,
        }
    }

    /// When running in single binary mode, the resolver will use memory channels to
    /// communicate with the p2p and store modules.
    /// The gateway itself is exposing a grpc endpoint to be also usable
    /// as a single entry point for other system services.
    pub fn default_rpc_config() -> RpcClientConfig {
        RpcClientConfig {
            gateway_addr: RpcClientConfig::default_network().gateway_addr,
            p2p_addr: None,
            store_addr: None,
            channels: Some(1),
        }
    }

    // synchronize the top level configs across subsystems
    pub fn synchronize_subconfigs(&mut self) {
        self.gateway.rpc_client = self.rpc_client.clone();
        self.p2p.rpc_client = self.rpc_client.clone();
        self.store.rpc_client = self.rpc_client.clone();
    }
}

impl Default for Config {
    fn default() -> Self {
        #[cfg(all(feature = "http-uds-gateway", unix))]
        let gateway_uds_path: PathBuf = tempfile::Builder::new()
            .prefix("beetle")
            .tempfile()
            .unwrap()
            .path()
            .join("ipfsd.http");
        let rpc_client = Self::default_rpc_config();
        let metrics_config = MetricsConfig::default();
        let store_config = default_store_config(None, rpc_client.clone()).unwrap();
        let key_store_path = beetle_util::beetle_data_root().unwrap();
        Self {
            rpc_client: rpc_client.clone(),
            metrics: metrics_config,
            gateway: beetle_gateway::config::Config::default(),
            store: store_config,
            p2p: default_p2p_config(rpc_client, key_store_path),
            #[cfg(all(feature = "http-uds-gateway", unix))]
            gateway_uds_path: Some(gateway_uds_path),
        }
    }
}

fn default_store_config(
    store_path: Option<PathBuf>,
    ipfsd: RpcClientConfig,
) -> Result<beetle_store::config::Config> {
    let path = config_data_path(store_path)?;
    Ok(beetle_store::config::Config {
        path,
        rpc_client: ipfsd,
    })
}

fn default_p2p_config(
    ipfsd: RpcClientConfig,
    key_store_path: PathBuf,
) -> beetle_p2p::config::Config {
    beetle_p2p::config::Config {
        key_store_path,
        libp2p: Libp2pConfig::default(),
        rpc_client: ipfsd,
    }
}

impl Source for Config {
    fn clone_into_box(&self) -> Box<dyn Source + Send + Sync> {
        Box::new(self.clone())
    }

    fn collect(&self) -> Result<Map<String, Value>, ConfigError> {
        let mut map: Map<String, Value> = Map::new();

        insert_into_config_map(&mut map, "gateway", self.gateway.collect()?);
        insert_into_config_map(&mut map, "store", self.store.collect()?);
        insert_into_config_map(&mut map, "p2p", self.p2p.collect()?);
        insert_into_config_map(&mut map, "rpc_client", self.rpc_client.collect()?);
        insert_into_config_map(&mut map, "metrics", self.metrics.collect()?);
        #[cfg(all(feature = "http-uds-gateway", unix))]
        if let Some(uds_path) = self.gateway_uds_path.as_ref() {
            insert_into_config_map(
                &mut map,
                "gateway_uds_path",
                uds_path.to_str().unwrap().to_string(),
            );
        }
        Ok(map)
    }
}

impl beetle_gateway::handlers::StateConfig for Config {
    fn rpc_client(&self) -> &beetle_rpc_client::Config {
        &self.rpc_client
    }

    fn public_url_base(&self) -> &str {
        &self.gateway.public_url_base
    }

    fn port(&self) -> u16 {
        self.gateway.port
    }

    fn user_headers(&self) -> &HeaderMap<HeaderValue> {
        &self.gateway.headers
    }

    fn redirect_to_subdomain(&self) -> bool {
        self.gateway.redirect_to_subdomain
    }
}
