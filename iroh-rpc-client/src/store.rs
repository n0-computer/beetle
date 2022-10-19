use anyhow::Result;
use bytes::{Bytes, BytesMut};
use cid::Cid;
use tarpc::context::Context;

impl_client!(Store);

impl StoreClient {
    pub async fn version(&self) -> Result<String> {
        let res = self.backend().await?.version(Context::current()).await??;
        Ok(res)
    }

    pub async fn put(&self, cid: Cid, blob: Bytes, links: Vec<Cid>) -> Result<()> {
        self.backend()
            .await?
            .put(Context::current(), cid, blob, links)
            .await??;
        Ok(())
    }

    pub async fn get(&self, cid: Cid) -> Result<Option<BytesMut>> {
        let res = self.backend().await?.get(Context::current(), cid).await??;
        Ok(res)
    }

    pub async fn has(&self, cid: Cid) -> Result<bool> {
        let res = self.backend().await?.has(Context::current(), cid).await??;
        Ok(res)
    }

    pub async fn get_links(&self, cid: Cid) -> Result<Option<Vec<Cid>>> {
        let links = self
            .backend()
            .await?
            .get_links(Context::current(), cid)
            .await??;

        if links.is_empty() {
            Ok(None)
        } else {
            Ok(Some(links))
        }
    }

    pub async fn get_size(&self, cid: Cid) -> Result<Option<u64>> {
        let size = self
            .backend()
            .await?
            .get_size(Context::current(), cid)
            .await??;
        Ok(size)
    }
}
