// #[cfg(feature = "grpc")]
// use crate::status::{self, StatusRow};
use anyhow::Result;

impl_client!(Gateway);

impl GatewayClient {
    pub async fn version(&self) -> Result<String> {
        let res = self
            .backend()
            .await?
            .version(tarpc::context::Context::current())
            .await??;
        Ok(res)
    }
}
