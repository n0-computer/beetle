use std::ops::Range;
use std::pin::Pin;
use std::task::Poll;

use anyhow::Result;
use bytes::Bytes;
use cid::Cid;
use futures::{StreamExt, TryStream};
use http::{HeaderMap, StatusCode};
use iroh_car::{CarHeader, CarWriter};
use iroh_metrics::{
    core::{MObserver, MRecorder},
    gateway::{GatewayHistograms, GatewayMetrics},
    observe, record,
    resolver::OutMetrics,
};
use iroh_resolver::dns_resolver::Config;
use iroh_resolver::resolver::{Metadata, Out, OutPrettyReader, OutType, Resolver};
use iroh_unixfs::{content_loader::ContentLoader, Source};
use mime::Mime;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, AsyncWrite};
use tokio_util::io::ReaderStream;
use tracing::{info, warn};

use crate::constants::RECURSION_LIMIT;
use crate::error::GatewayError;
use crate::handler_params::GetParams;
use crate::ipfs_request::IpfsRequest;
use crate::response::ResponseFormat;

#[derive(Debug, Clone)]
pub struct Client<T: ContentLoader> {
    pub(crate) resolver: Resolver<T>,
}

#[derive(Debug)]
pub struct PrettyStreamBody<T: ContentLoader>(
    ReaderStream<tokio::io::BufReader<OutPrettyReader<T>>>,
    Option<u64>,
    Option<Mime>,
);

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum FileResult<T: ContentLoader> {
    File(PrettyStreamBody<T>),
    Directory(Out),
    Raw(PrettyStreamBody<T>),
}

impl<T: ContentLoader> PrettyStreamBody<T> {
    pub fn get_mime(&self) -> Option<Mime> {
        self.2.clone()
    }

    pub fn get_size(&self) -> Option<u64> {
        self.1
    }
}

impl<T: ContentLoader + std::marker::Unpin> http_body::Body for PrettyStreamBody<T> {
    type Data = Bytes;
    type Error = String;

    fn poll_data(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        let stream = Pin::new(&mut self.0);
        match stream.try_poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Ok(chunk))) => Poll::Ready(Some(Ok(chunk))),
            Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err.to_string()))),
            Poll::Ready(None) => Poll::Ready(None),
        }
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }

    fn size_hint(&self) -> http_body::SizeHint {
        let mut size_hint = http_body::SizeHint::new();
        if let Some(size) = self.1 {
            size_hint.set_exact(size);
        }
        size_hint
    }
}

impl<T: ContentLoader + std::marker::Unpin> Client<T> {
    pub fn new(rpc_client: &T, dns_resolver_config: Config) -> Self {
        Self {
            resolver: Resolver::with_dns_resolver(rpc_client.clone(), dns_resolver_config),
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn build_ipfs_request(
        &self,
        path: &iroh_resolver::resolver::Path,
        query_params: &GetParams,
        format: ResponseFormat,
        subdomain_mode: bool,
    ) -> Result<IpfsRequest, GatewayError> {
        info!("build ipfs request {}", path);
        let path_metadata = match self
            .retrieve_path_metadata(path.clone(), format == ResponseFormat::Raw)
            .await
        {
            Ok(metadata) => metadata,
            Err(e) => {
                if e == "offline" {
                    return Err(GatewayError::new(StatusCode::SERVICE_UNAVAILABLE, &e));
                } else if e.starts_with("failed to find") {
                    return Err(GatewayError::new(StatusCode::NOT_FOUND, &e));
                } else {
                    return Err(GatewayError::new(StatusCode::INTERNAL_SERVER_ERROR, &e));
                }
            }
        };
        Ok(IpfsRequest {
            format,
            cid: path.root().clone(),
            resolved_path: path.clone(),
            query_params: query_params.clone(),
            subdomain_mode,
            path_metadata,
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn retrieve_path_metadata(
        &self,
        path: iroh_resolver::resolver::Path,
        raw_format: bool,
    ) -> Result<Out, String> {
        info!("retrieve path metadata {}", path);
        if raw_format {
            self.resolver
                .resolve_raw(path)
                .await
                .map_err(|e| e.to_string())
        } else {
            self.resolver.resolve(path).await.map_err(|e| e.to_string())
        }
    }

    #[tracing::instrument(skip(self, path_metadata))]
    pub async fn get_file(
        &self,
        path: iroh_resolver::resolver::Path,
        path_metadata: Option<Out>,
        start_time: std::time::Instant,
        range: Option<Range<u64>>,
    ) -> Result<(FileResult<T>, Metadata), String> {
        info!("get file {}", path);
        let path_metadata = if let Some(path_metadata) = path_metadata {
            path_metadata
        } else {
            self.retrieve_path_metadata(path.clone(), false).await?
        };
        let metadata = path_metadata.metadata().clone();
        record_ttfb_metrics(start_time, &metadata.source);

        if path_metadata.is_dir() {
            let body = FileResult::Directory(path_metadata);
            Ok((body, metadata))
        } else {
            let reader = path_metadata
                .pretty(
                    self.resolver.clone(),
                    OutMetrics { start: start_time },
                    range.as_ref().map(|range| range.end as usize),
                )
                .map_err(|e| e.to_string())?;

            let mut buf_reader = tokio::io::BufReader::with_capacity(1024 * 1024, reader);
            let body_sample = buf_reader.fill_buf().await.map_err(|e| e.to_string())?;
            let mime = sniff_content_type(body_sample);
            if let Some(range) = range {
                buf_reader
                    .seek(tokio::io::SeekFrom::Start(range.start))
                    .await
                    .map_err(|e| e.to_string())?;
            }
            let stream = ReaderStream::new(buf_reader);

            let body = PrettyStreamBody(stream, metadata.size, Some(mime));

            if metadata.typ == OutType::Raw {
                return Ok((FileResult::Raw(body), metadata));
            }

            Ok((FileResult::File(body), metadata))
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_car_recursive(
        self,
        path: iroh_resolver::resolver::Path,
        start_time: std::time::Instant,
    ) -> Result<axum::body::StreamBody<ReaderStream<tokio::io::DuplexStream>>, String> {
        info!("get car {}", path);
        // TODO: Find out what a good buffer size is here.
        let (writer, reader) = tokio::io::duplex(1024 * 64);
        let body = axum::body::StreamBody::new(ReaderStream::new(reader));
        let client = self.clone();
        tokio::task::spawn(async move {
            if let Err(e) = fetch_car_recursive(&client.resolver, path, writer, start_time).await {
                warn!("failed to load recursively: {:?}", e);
            }
        });

        Ok(body)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_file_recursive(
        self,
        path: iroh_resolver::resolver::Path,
        start_time: std::time::Instant,
    ) -> Result<axum::body::Body, String> {
        info!("get file {}", path);
        let (mut sender, body) = axum::body::Body::channel();

        tokio::spawn(async move {
            let res = self.resolver.resolve_recursive(path);
            tokio::pin!(res);

            while let Some(res) = res.next().await {
                match res {
                    Ok(res) => {
                        let metadata = res.metadata().clone();
                        record_ttfb_metrics(start_time, &metadata.source);
                        let reader = res.pretty(
                            self.resolver.clone(),
                            OutMetrics { start: start_time },
                            None,
                        );
                        match reader {
                            Ok(mut reader) => {
                                let mut bytes = Vec::new();
                                reader.read_to_end(&mut bytes).await.unwrap();
                                sender.send_data(bytes.into()).await.unwrap();
                            }
                            Err(e) => {
                                warn!("failed to load recursively: {:?}", e);
                                sender.abort();
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("failed to load recursively: {:?}", e);
                        sender.abort();
                        break;
                    }
                }
            }
        });

        Ok(body)
    }
}

impl<T: ContentLoader> Client<T> {
    #[tracing::instrument(skip(self))]
    pub async fn has_file_locally(&self, cid: &Cid) -> Result<bool> {
        info!("has cid {}", cid);
        self.resolver.has_cid(cid).await
    }
}

async fn fetch_car_recursive<T, W>(
    resolver: &Resolver<T>,
    path: iroh_resolver::resolver::Path,
    writer: W,
    start_time: std::time::Instant,
) -> Result<(), anyhow::Error>
where
    T: ContentLoader,
    W: AsyncWrite + Send + Unpin,
{
    let stream = resolver.resolve_recursive_raw(path, Some(RECURSION_LIMIT));
    tokio::pin!(stream);

    let root = stream
        .next()
        .await
        .ok_or_else(|| anyhow::anyhow!("root cid not found"))??;

    let header = CarHeader::new_v1(vec![*root.cid()]);
    let mut writer = CarWriter::new(header, writer);
    writer.write(*root.cid(), root.content()).await?;

    while let Some(block) = stream.next().await {
        let block = block?;
        record_ttfb_metrics(start_time, block.source());
        writer.write(*block.cid(), block.content()).await?;
    }
    Ok(())
}

fn record_ttfb_metrics(start_time: std::time::Instant, source: &Source) {
    record!(
        GatewayMetrics::TimeToFetchFirstBlock,
        start_time.elapsed().as_millis() as u64
    );
    match *source {
        Source::Store(_) => observe!(
            GatewayHistograms::TimeToFetchFirstBlockCached,
            start_time.elapsed().as_millis() as f64
        ),
        _ => observe!(
            GatewayHistograms::TimeToFetchFirstBlock,
            start_time.elapsed().as_millis() as f64
        ),
    }
}

pub(crate) fn sniff_content_type(body_sample: &[u8]) -> Mime {
    let classifier = mime_classifier::MimeClassifier::new();
    let context = mime_classifier::LoadContext::Browsing;
    let no_sniff_flag = mime_classifier::NoSniffFlag::Off;
    let apache_bug_flag = mime_classifier::ApacheBugFlag::On;
    let supplied_type = None;
    classifier.classify(
        context,
        no_sniff_flag,
        apache_bug_flag,
        &supplied_type,
        body_sample,
    )
}
