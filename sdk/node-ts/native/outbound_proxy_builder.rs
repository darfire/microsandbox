use napi::bindgen_prelude::*;
use napi_derive::napi;

use microsandbox_network::{
    OutboundProxy, OutboundProxyBuilder as RustOutboundProxyBuilder, OutboundProxyConfig,
    Socks5ProxyBuilder as RustSocks5ProxyBuilder,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Selects the protocol for an outbound proxy.
#[napi(js_name = "OutboundProxyBuilder")]
pub struct JsOutboundProxyBuilder {
    inner: Option<RustOutboundProxyBuilder>,
}

/// Builds a SOCKS5 outbound proxy.
#[napi(js_name = "Socks5ProxyBuilder")]
pub struct JsSocks5ProxyBuilder {
    inner: Option<RustSocks5ProxyBuilder>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

#[napi]
impl JsOutboundProxyBuilder {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Some(RustOutboundProxyBuilder::new()),
        }
    }

    /// Select a SOCKS5 proxy at `address`.
    #[napi]
    pub fn socks5(&mut self, address: String) -> Result<JsSocks5ProxyBuilder> {
        let builder = self
            .inner
            .take()
            .ok_or_else(|| napi::Error::from_reason("OutboundProxyBuilder already consumed"))?;
        Ok(JsSocks5ProxyBuilder {
            inner: Some(builder.socks5(address)),
        })
    }
}

impl JsSocks5ProxyBuilder {
    pub(crate) fn take_built(&mut self) -> Result<OutboundProxy> {
        self.inner
            .take()
            .ok_or_else(|| napi::Error::from_reason("Socks5ProxyBuilder already consumed"))?
            .build()
            .map_err(|error| napi::Error::from_reason(error.to_string()))
    }
}
