use std::{
    fmt::{Debug, Display},
    net::SocketAddr,
    str::FromStr,
};

use anyhow::{anyhow, bail};
use serde_with::{DeserializeFromStr, SerializeDisplay};

#[derive(SerializeDisplay, DeserializeFromStr)]
pub enum Addr<Req = (), Resp = ()> {
    #[cfg(feature = "grpc")]
    GrpcHttp2(SocketAddr),
    #[cfg(all(feature = "grpc", unix))]
    GrpcUds(std::path::PathBuf),
    #[cfg(feature = "mem")]
    Mem(tarpc::transport::channel::Channel<Req, Resp>),
}

impl<Req, Resp> Clone for Addr<Req, Resp> {
    fn clone(&self) -> Self {
        match self {
            #[cfg(feature = "grpc")]
            Self::GrpcHttp2(addr) => Self::GrpcHttp2(addr.clone()),
            #[cfg(all(feature = "grpc", unix))]
            Self::GrpcUds(path) => Self::GrpcUds(path.clone()),
            #[cfg(feature = "mem")]
            Self::Mem(_) => panic!("can't clone mem addrs"),
        }
    }
}

impl<Req, Resp> PartialEq for Addr<Req, Resp> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            #[cfg(feature = "grpc")]
            (Self::GrpcHttp2(addr1), Self::GrpcHttp2(addr2)) => addr1.eq(addr2),
            #[cfg(all(feature = "grpc", unix))]
            (Self::GrpcUds(path1), Self::GrpcUds(path2)) => path1.eq(path2),
            _ => false,
        }
    }
}

impl<Req, Resp> Addr<Req, Resp> {
    pub fn new_mem() -> (Addr<Req, Resp>, Addr<Resp, Req>) {
        let (s, r) = tarpc::transport::channel::bounded(256);

        (Addr::Mem(r), Addr::Mem(s))
    }
}

impl<Req, Resp> Addr<Req, Resp> {
    pub fn try_as_socket_addr(&self) -> Option<SocketAddr> {
        #[cfg(feature = "grpc")]
        if let Addr::GrpcHttp2(addr) = self {
            return Some(*addr);
        }
        None
    }
}

impl<Req, Resp> Display for Addr<Req, Resp> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "grpc")]
            Addr::GrpcHttp2(addr) => write!(f, "grpc://{}", addr),
            #[cfg(all(feature = "grpc", unix))]
            Addr::GrpcUds(path) => write!(f, "grpc://{}", path.display()),
            #[cfg(feature = "mem")]
            Addr::Mem(_) => write!(f, "mem"),
            #[allow(unreachable_patterns)]
            _ => unreachable!(),
        }
    }
}

impl<Req, Resp> Debug for Addr<Req, Resp> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl<Req, Resp> FromStr for Addr<Req, Resp> {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        #[cfg(feature = "mem")]
        if s == "mem" {
            bail!("memory addresses can not be serialized or deserialized");
        }

        #[cfg(feature = "grpc")]
        {
            let mut parts = s.split("://");
            if let Some(prefix) = parts.next() {
                if prefix == "grpc" {
                    if let Some(part) = parts.next() {
                        if let Ok(addr) = part.parse::<SocketAddr>() {
                            return Ok(Addr::GrpcHttp2(addr));
                        }
                        #[cfg(unix)]
                        if let Ok(path) = part.parse::<std::path::PathBuf>() {
                            return Ok(Addr::GrpcUds(path));
                        }
                    }
                }
            }
        }

        Err(anyhow!("invalid addr: {}", s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "grpc")]
    #[test]
    fn test_addr_roundtrip_grpc_http2() {
        let socket: SocketAddr = "198.168.2.1:1234".parse().unwrap();
        let addr = Addr::GrpcHttp2(socket);

        assert_eq!(addr.to_string().parse::<Addr>().unwrap(), addr);
        assert_eq!(addr.to_string(), "grpc://198.168.2.1:1234");
    }

    #[cfg(all(feature = "grpc", unix))]
    #[test]
    fn test_addr_roundtrip_grpc_uds() {
        let path: std::path::PathBuf = "/foo/bar".parse().unwrap();
        let addr = Addr::GrpcUds(path);

        assert_eq!(addr.to_string().parse::<Addr>().unwrap(), addr);
        assert_eq!(addr.to_string(), "grpc:///foo/bar");
    }
}
