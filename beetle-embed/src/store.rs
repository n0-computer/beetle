//! Store services to use in an beetle system.

use std::path::PathBuf;

use anyhow::Result;
use beetle_one::mem_store;
use beetle_rpc_types::store::StoreAddr;
use beetle_rpc_types::Addr;
use beetle_store::Config as StoreConfig;
use tokio::task::JoinHandle;

/// A beetle store backed by an on-disk RocksDB.
///
/// An beetle system needs a store service for keeping local state and IPFS data.  This one
/// uses RocksDB in a directory on disk.
#[derive(Debug)]
pub struct RocksStoreService {
    task: JoinHandle<()>,
    addr: StoreAddr,
}

impl RocksStoreService {
    /// Starts a new beetle Store service with RocksDB storage.
    ///
    /// This implicitly starts a task on the tokio runtime to manage the storage node.
    pub async fn new(path: PathBuf) -> Result<Self> {
        let addr = Addr::new_mem();
        let config = StoreConfig::with_rpc_addr(path, addr.clone());
        let task = mem_store::start(addr.clone(), config).await?;
        Ok(Self { task, addr })
    }

    /// Returns the internal RPC address of this store node.
    ///
    /// This is used by the other beetle services, like the p2p and gateway services, to use
    /// the store.
    pub fn addr(&self) -> StoreAddr {
        self.addr.clone()
    }

    /// Stop this store service.
    ///
    /// This function waits for the store to be fully terminated and only returns once it is
    /// no longer running.
    // TODO: This should be graceful termination.
    pub async fn stop(mut self) -> Result<()> {
        // This dummy task will be aborted by Drop.
        let fut = futures::future::ready(());
        let dummy_task = tokio::spawn(fut);
        let task = std::mem::replace(&mut self.task, dummy_task);

        task.abort();

        // Because we currently don't do graceful termination we expect a cancelled error.
        match task.await {
            Ok(()) => Ok(()),
            Err(err) if err.is_cancelled() => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

impl Drop for RocksStoreService {
    fn drop(&mut self) {
        // Abort the task without polling it.  It may or may not ever be polled again and
        // actually abort.  If .stop() has been called though the task is already shut down
        // gracefully and not polling it anymore has no significance.
        self.task.abort();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use testdir::testdir;
    use tokio::time;

    use super::*;

    #[tokio::test]
    async fn test_create_store_stop() {
        let dir = testdir!();
        let marker = dir.join("CURRENT");

        let store = RocksStoreService::new(dir).await.unwrap();
        assert!(marker.exists());

        let fut = store.stop();
        let ret = time::timeout(Duration::from_millis(500), fut).await;

        assert!(ret.is_ok());
    }
}
