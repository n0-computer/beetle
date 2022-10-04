use std::sync::Arc;

use anyhow::{anyhow, Result};
use clap::Parser;
use iroh_gateway::{
    bad_bits::{self, BadBits},
    cli::Args,
    config::{Config, CONFIG_FILE_NAME, ENV_PREFIX},
    core::Core,
    metrics,
};
#[cfg(target_os = "linux")]
use iroh_resolver::resolver::ContentLoader;
use iroh_rpc_client::Client as RpcClient;
use iroh_util::{iroh_config_path, make_config};
use tokio::sync::RwLock;
use tracing::{debug, error};

#[cfg(target_os = "linux")]
async fn serve<T: ContentLoader + std::marker::Unpin>(_: usize, core: Core<T>) {
    let server = core.server();
    println!("listening on {}", server.local_addr());
    server.await.unwrap();
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args = Args::parse();

    let cfg_path = iroh_config_path(CONFIG_FILE_NAME)?;
    let sources = vec![Some(cfg_path), args.cfg.clone()];
    let mut config = make_config(
        // default
        Config::default(),
        // potential config files
        sources,
        // env var prefix for this config
        ENV_PREFIX,
        // map of present command line arguments
        args.make_overrides_map(),
    )
    .unwrap();
    config.metrics = metrics::metrics_config_with_compile_time_info(config.metrics);
    println!("{:#?}", config);

    let metrics_config = config.metrics.clone();
    let bad_bits = match config.denylist {
        true => Arc::new(Some(RwLock::new(BadBits::new()))),
        false => Arc::new(None),
    };
    let rpc_addr = config
        .server_rpc_addr()?
        .ok_or_else(|| anyhow!("missing gateway rpc addr"))?;
    let content_loader = RpcClient::new(config.rpc_client.clone()).await?;
    let handler = Core::new(
        Arc::new(config),
        rpc_addr,
        Arc::clone(&bad_bits),
        content_loader,
    )
    .await?;

    let bad_bits_handle = bad_bits::spawn_bad_bits_updater(Arc::clone(&bad_bits));

    let metrics_handle = iroh_metrics::MetricsHandle::new(metrics_config)
        .await
        .expect("failed to initialize metrics");

    #[cfg(unix)]
    {
        match iroh_util::increase_fd_limit() {
            Ok(soft) => debug!("NOFILE limit: soft = {}", soft),
            Err(err) => error!("Error increasing NOFILE limit: {}", err),
        }
    }

    let mut handlers = Vec::new();
    #[cfg(target_os = "linux")]
    {
        for i in 0..num_cpus::get() {
            let hc = handler.clone();
            let h = std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(serve(i, hc));
            });
            handlers.push(h);
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        let server = handler.server();
        println!("listening on {}", server.local_addr());
        let core_task = tokio::spawn(async move {
            server.await.unwrap();
        });
        handlers.push(core_task);
    }

    iroh_util::block_until_sigint().await;

    #[cfg(not(target_os = "linux"))]
    for h in handlers {
        h.abort();
    }

    metrics_handle.shutdown();
    if let Some(handle) = bad_bits_handle {
        handle.abort();
    }

    Ok(())
}
