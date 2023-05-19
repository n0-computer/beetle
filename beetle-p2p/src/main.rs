use anyhow::{anyhow, Context, Result};
use beetle_p2p::config::{Config, CONFIG_FILE_NAME, ENV_PREFIX};
use beetle_p2p::ServerConfig;
use beetle_p2p::{cli::Args, metrics, DiskStorage, Keychain, Node};
use beetle_util::lock::ProgramLock;
use beetle_util::{beetle_config_path, make_config};
use clap::Parser;
use tokio::task;
use tracing::error;

/// Starts daemon process
fn main() -> Result<()> {
    let mut lock = ProgramLock::new("beetle-p2p")?;
    lock.acquire_or_exit();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .max_blocking_threads(2048)
        .thread_stack_size(16 * 1024 * 1024)
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async move {
        let version = option_env!("BEETLE_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"));
        println!("Starting beetle-p2p, version {version}");

        let args = Args::parse();

        // TODO: configurable network
        let cfg_path = beetle_config_path(CONFIG_FILE_NAME)?;
        let sources = [Some(cfg_path.as_path()), args.cfg.as_deref()];
        let network_config = make_config(
            // default
            ServerConfig::default(),
            // potential config files
            &sources,
            // env var prefix for this config
            ENV_PREFIX,
            // map of present command line arguments
            args.make_overrides_map(),
        )
        .context("invalid config")?;

        let metrics_config =
            metrics::metrics_config_with_compile_time_info(network_config.metrics.clone());

        let metrics_handle = beetle_metrics::MetricsHandle::new(metrics_config)
            .await
            .map_err(|e| anyhow!("metrics init failed: {:?}", e))?;

        #[cfg(unix)]
        {
            match beetle_util::increase_fd_limit() {
                Ok(soft) => tracing::debug!("NOFILE limit: soft = {}", soft),
                Err(err) => error!("Error increasing NOFILE limit: {}", err),
            }
        }

        let network_config = Config::from(network_config);
        let kc = Keychain::<DiskStorage>::new(network_config.key_store_path.clone()).await?;
        let rpc_addr = network_config
            .rpc_addr()
            .ok_or_else(|| anyhow!("missing p2p rpc addr"))?;
        let mut p2p = Node::new(network_config, rpc_addr, kc).await?;

        // Start services
        let p2p_task = task::spawn(async move {
            if let Err(err) = p2p.run().await {
                error!("{:?}", err);
            }
        });

        beetle_util::block_until_sigint().await;

        // Cancel all async services
        p2p_task.abort();
        p2p_task.await.ok();

        metrics_handle.shutdown();
        Ok(())
    })
}
