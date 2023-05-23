use beetle_metrics::config::Config as MetricsConfig;
use beetle_rpc_client::Config as RpcClientConfig;
use beetle_unixfs::indexer::IndexerUrl;
use beetle_util::insert_into_config_map;
use config::{ConfigError, Map, Source, Value};
use serde::{Deserialize, Serialize};

/// CONFIG_FILE_NAME is the name of the optional config file located in the beetle home directory
pub const CONFIG_FILE_NAME: &str = "ctl.config.toml";
/// ENV_PREFIX should be used along side the config field name to set a config field using
/// environment variables
pub const ENV_PREFIX: &str = "BEETLE_CTL";

/// Configuration for [`beetle-api`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub rpc_client: RpcClientConfig,
    pub metrics: MetricsConfig,
    pub http_resolvers: Option<Vec<String>>,
    pub indexer_endpoint: Option<IndexerUrl>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rpc_client: RpcClientConfig::default_network(),
            metrics: Default::default(),
            http_resolvers: None,
            indexer_endpoint: Some(IndexerUrl::default()),
        }
    }
}

impl Source for Config {
    fn clone_into_box(&self) -> Box<dyn Source + Send + Sync> {
        Box::new(self.clone())
    }
    fn collect(&self) -> Result<Map<String, Value>, ConfigError> {
        let mut map: Map<String, Value> = Map::new();
        insert_into_config_map(&mut map, "rpc_client", self.rpc_client.collect()?);
        insert_into_config_map(&mut map, "metrics", self.metrics.collect()?);
        if let Some(http_resolvers) = &self.http_resolvers {
            insert_into_config_map(&mut map, "http_resolvers", http_resolvers.clone());
        }
        if let Some(indexer_endpoint) = &self.indexer_endpoint {
            insert_into_config_map(&mut map, "indexer_endpoint", indexer_endpoint.clone());
        }

        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::Config as ConfigBuilder;

    #[test]
    fn test_collect() {
        let default = Config::default();
        let mut expect: Map<String, Value> = Map::new();
        expect.insert(
            "rpc_client".to_string(),
            Value::new(None, default.rpc_client.collect().unwrap()),
        );
        expect.insert(
            "metrics".to_string(),
            Value::new(None, default.metrics.collect().unwrap()),
        );
        expect.insert(
            "indexer_endpoint".to_string(),
            Value::new(None, default.indexer_endpoint.clone()),
        );

        expect.insert(
            "http_resolvers".to_string(),
            Value::new(None, default.http_resolvers.clone()),
        );

        let got = default.collect().unwrap();

        for key in got.keys() {
            let left = expect.get(key).unwrap();
            let right = got.get(key).unwrap();
            assert_eq!(left, right);
        }
    }

    #[test]
    fn test_build_config_from_struct() {
        let expect = Config::default();
        let got: Config = ConfigBuilder::builder()
            .add_source(expect.clone())
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        assert_eq!(expect, got);
    }
}
