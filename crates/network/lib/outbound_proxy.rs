//! Outbound proxy configuration, builders, parsing, and connection dispatch.

use std::fmt;
use std::io;
use std::net::{AddrParseError, SocketAddr};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_socks::tcp::Socks5Stream;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Proxy used for outbound sandbox connections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "protocol", content = "address", rename_all = "lowercase")]
#[non_exhaustive]
pub enum OutboundProxy {
    /// A SOCKS5 proxy at the given address.
    Socks5(SocketAddr),
}

/// Selects the protocol for an [`OutboundProxy`].
#[derive(Debug, Clone, Copy, Default)]
pub struct OutboundProxyBuilder;

/// Builds a SOCKS5 outbound proxy.
#[derive(Debug, Clone)]
pub struct Socks5ProxyBuilder {
    address: String,
}

/// Error returned when building an outbound proxy.
#[derive(Debug, Clone, thiserror::Error)]
pub enum OutboundProxyBuildError {
    /// The proxy address is not a valid socket address.
    #[error("invalid SOCKS5 proxy address {address:?}: {source}")]
    InvalidAddress {
        /// Invalid address text.
        address: String,
        /// Socket-address parsing failure.
        #[source]
        source: AddrParseError,
    },
}

/// Error returned when parsing an outbound proxy URI.
///
/// URI parsing is intended for string-only interfaces such as the CLI. SDKs
/// should use [`OutboundProxyBuilder`] instead.
#[derive(Debug, Clone, thiserror::Error)]
pub enum OutboundProxyParseError {
    /// The URI does not include a `scheme://` prefix.
    #[error("outbound proxy URI must include a protocol, for example socks5://127.0.0.1:1080")]
    MissingProtocol,

    /// The URI uses a proxy protocol that is not supported yet.
    #[error(
        "unsupported outbound proxy protocol {protocol:?}; currently only socks5:// is supported"
    )]
    UnsupportedProtocol {
        /// Unsupported URI scheme.
        protocol: String,
    },

    /// The URI includes credentials, which are not supported.
    #[error("outbound proxy credentials are not supported in the URI")]
    CredentialsNotSupported,

    /// The URI includes a path, query, or fragment.
    #[error("outbound proxy URI must not include a path, query, or fragment")]
    ExtraComponentsNotSupported,

    /// The proxy address is not a valid socket address.
    #[error(transparent)]
    Build(#[from] OutboundProxyBuildError),
}

/// Converts a protocol-specific proxy builder into an [`OutboundProxy`].
///
/// Sandbox-facing builders use this trait to finalize protocol-specific proxy
/// builders and collect their validation errors.
#[doc(hidden)]
pub trait OutboundProxyConfig {
    /// Builds the outbound proxy.
    fn build(self) -> Result<OutboundProxy, OutboundProxyBuildError>;
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl OutboundProxy {
    /// Connects to `destination` through this outbound proxy.
    pub(crate) async fn connect(&self, destination: SocketAddr) -> io::Result<TcpStream> {
        match self {
            Self::Socks5(address) => Socks5Stream::connect(*address, destination)
                .await
                .map(|stream| stream.into_inner())
                .map_err(io::Error::other),
        }
    }
}

impl OutboundProxyBuilder {
    /// Creates a protocol selector.
    pub fn new() -> Self {
        Self
    }

    /// Starts building a SOCKS5 outbound proxy.
    pub fn socks5(self, address: impl Into<String>) -> Socks5ProxyBuilder {
        Socks5ProxyBuilder {
            address: address.into(),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl OutboundProxyConfig for Socks5ProxyBuilder {
    fn build(self) -> Result<OutboundProxy, OutboundProxyBuildError> {
        let address =
            self.address
                .parse()
                .map_err(|source| OutboundProxyBuildError::InvalidAddress {
                    address: self.address,
                    source,
                })?;
        Ok(OutboundProxy::Socks5(address))
    }
}

impl OutboundProxyConfig for OutboundProxy {
    fn build(self) -> Result<OutboundProxy, OutboundProxyBuildError> {
        Ok(self)
    }
}

impl fmt::Display for OutboundProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Socks5(address) => write!(f, "socks5://{address}"),
        }
    }
}

impl FromStr for OutboundProxy {
    type Err = OutboundProxyParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let (protocol, address) = raw
            .split_once("://")
            .ok_or(OutboundProxyParseError::MissingProtocol)?;
        match protocol {
            "socks5" => {}
            protocol => {
                return Err(OutboundProxyParseError::UnsupportedProtocol {
                    protocol: protocol.to_string(),
                });
            }
        };
        if address.contains('@') {
            return Err(OutboundProxyParseError::CredentialsNotSupported);
        }
        if address.contains(['/', '?', '#']) {
            return Err(OutboundProxyParseError::ExtraComponentsNotSupported);
        }
        Ok(OutboundProxyBuilder::new().socks5(address).build()?)
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::{OutboundProxy, OutboundProxyBuilder, OutboundProxyConfig};

    #[test]
    fn builder_creates_socks5_proxy() {
        let proxy = OutboundProxyBuilder::new()
            .socks5("127.0.0.1:1080")
            .build()
            .unwrap();

        assert_eq!(
            proxy,
            OutboundProxy::Socks5("127.0.0.1:1080".parse().unwrap())
        );
    }

    #[test]
    fn uri_parses_and_formats_for_cli() {
        let proxy: OutboundProxy = "socks5://127.0.0.1:1080".parse().unwrap();

        assert_eq!(
            proxy,
            OutboundProxy::Socks5("127.0.0.1:1080".parse().unwrap())
        );
        assert_eq!(proxy.to_string(), "socks5://127.0.0.1:1080");
    }

    #[test]
    fn uri_rejects_unsupported_forms() {
        for raw in [
            "127.0.0.1:1080",
            "socks4://127.0.0.1:1080",
            "socks5://user@127.0.0.1:1080",
            "socks5://127.0.0.1:1080/path",
            "socks5://127.0.0.1:1080?option=value",
            "socks5://127.0.0.1:1080#fragment",
        ] {
            assert!(raw.parse::<OutboundProxy>().is_err(), "accepted {raw:?}");
        }
    }

    #[tokio::test]
    async fn connects_through_socks5_proxy() {
        let target: SocketAddr = "93.184.216.34:443".parse().unwrap();
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        let proxy_task = tokio::spawn(async move {
            let (mut client, _) = proxy_listener.accept().await.unwrap();

            let mut greeting = [0u8; 3];
            client.read_exact(&mut greeting).await.unwrap();
            assert_eq!(greeting, [0x05, 0x01, 0x00]);
            client.write_all(&[0x05, 0x00]).await.unwrap();

            let mut request = [0u8; 10];
            client.read_exact(&mut request).await.unwrap();
            assert_eq!(request[0], 0x05, "SOCKS version");
            assert_eq!(request[1], 0x01, "CONNECT command");
            assert_eq!(request[3], 0x01, "IPv4 address type");
            assert_eq!(&request[4..8], &[93, 184, 216, 34]);
            assert_eq!(u16::from_be_bytes([request[8], request[9]]), 443);

            client
                .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await
                .unwrap();

            let mut buf = [0u8; 5];
            client.read_exact(&mut buf).await.unwrap();
            client.write_all(&buf).await.unwrap();
        });

        let mut stream = OutboundProxy::Socks5(proxy_addr)
            .connect(target)
            .await
            .unwrap();
        stream.write_all(b"hello").await.unwrap();
        let mut echoed = [0u8; 5];
        stream.read_exact(&mut echoed).await.unwrap();
        assert_eq!(&echoed, b"hello");

        proxy_task.await.unwrap();
    }
}
