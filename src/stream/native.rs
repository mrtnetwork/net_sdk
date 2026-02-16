pub struct StreamUtils;
use arti_client::{StreamPrefs, TorClient, config::TorClientConfigBuilder};
use log::debug;
use once_cell::sync::Lazy;
use rustls::{ClientConfig, RootCertStore, pki_types::ServerName};
use std::{fmt::Debug, path::Path, sync::Arc};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    sync::OnceCell,
};
use tokio_rustls::{TlsConnector, client::TlsStream};
use tor_rtcompat::PreferredRuntime;

use crate::{
    stream::tls::{CustomTlsVerifier, TofuVerifier},
    types::{
        AddressInfo,
        config::{NetConfig, NetConfigTor, NetHttpProtocol, NetProtocol, NetTlsMode},
        error::NetResultStatus,
    },
};

static TOR_CLIENT: OnceCell<TorClient<PreferredRuntime>> = OnceCell::const_new();

pub trait AsyncReadWrite: AsyncRead + AsyncWrite + Unpin + Send + Sync + Debug {}
impl<T> AsyncReadWrite for T where T: AsyncRead + AsyncWrite + Unpin + Send + Sync + Debug + 'static {}

static TLS_VERIFIER: Lazy<Arc<rustls::client::WebPkiServerVerifier>> = Lazy::new(|| {
    let mut root_store: RootCertStore = RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let builder = rustls::client::WebPkiServerVerifier::builder(Arc::new(root_store))
        .build()
        .unwrap();
    builder
});

impl StreamUtils {
    pub fn get_server_name(host: &str) -> Result<ServerName<'static>, NetResultStatus> {
        ServerName::try_from(host.to_owned()).map_err(|_| NetResultStatus::InvalidUrl)
    }
    pub fn tor_inited() -> bool {
        TOR_CLIENT.initialized()
    }
    pub async fn init_tor_config(config: &NetConfigTor) -> Result<(), NetResultStatus> {
        if TOR_CLIENT.initialized() {
            return Ok(());
        }
        let _ = TOR_CLIENT
            .get_or_try_init(|| async {
                let config = TorClientConfigBuilder::from_directories(
                    Path::new(&config.cache_dir),
                    Path::new(&config.state_dir),
                )
                .build()
                .map_err(|e| {
                    debug!("Tor client error: {:#?} ", e);
                    NetResultStatus::InvalidTorConfig
                })?;

                TorClient::create_bootstrapped(config).await.map_err(|e| {
                    debug!("create_bootstrapped error: {:#?} ", e);
                    NetResultStatus::TorInitializationFailed
                })
            })
            .await?;
        Ok(())
    }

    pub async fn get_tor_client() -> Result<TorClient<PreferredRuntime>, NetResultStatus> {
        let client = TOR_CLIENT.get();
        match client {
            Some(client) => Ok(client.clone()),
            None => Err(NetResultStatus::TorClientNotInitialized),
        }
    }

    pub fn create_tls_config(tls_mode: &NetTlsMode) -> Result<ClientConfig, NetResultStatus> {
        let tls = TLS_VERIFIER.clone();
        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(CustomTlsVerifier::new(
                tls,
                tls_mode.clone(),
            )))
            .with_no_client_auth();
        Ok(config)
    }
    pub fn create_no_verify_tls_config() -> Result<ClientConfig, NetResultStatus> {
        Ok(ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(TofuVerifier))
            .with_no_client_auth())
    }
    pub async fn create_tcp_stream(addr: &AddressInfo) -> Result<TcpStream, NetResultStatus> {
        TcpStream::connect((addr.host.to_string(), addr.port))
            .await
            .map_err(|e| {
                debug!("create_tcp_stream error: {:#?}, {:#?} ", e, addr.host);
                NetResultStatus::ConnectionError
            })
    }
    pub async fn create_tls_stream<T: AsyncReadWrite>(
        addr: &AddressInfo,
        stream: T,
        protocol: &NetProtocol,
        http_protocol: &Option<NetHttpProtocol>,
        tls_mode: &NetTlsMode,
    ) -> Result<TlsStream<T>, NetResultStatus> {
        let connector = StreamUtils::create_tls_connector(protocol, http_protocol, tls_mode)?;
        let domain = StreamUtils::get_server_name(&addr.host)?;
        let stream = connector
            .connect(domain, stream)
            .await
            .map_err(|_| NetResultStatus::TlsError)?;
        Ok(stream)
    }
    pub async fn create_data_stream(
        config: &NetConfig,
    ) -> Result<arti_client::DataStream, NetResultStatus> {
        let client = StreamUtils::get_tor_client().await?;
        let prefs = StreamPrefs::new();
        let stream = client
            .connect_with_prefs((config.addr.host.to_string(), config.addr.port), &prefs)
            .await
            .map_err(|e| {
                debug!("Tor connection error: {:#?} ", e);
                NetResultStatus::TorNetError
            });
        stream
    }

    pub fn create_tls_connector(
        protocol: &NetProtocol,
        http_protocol: &Option<NetHttpProtocol>,
        tls_mode: &NetTlsMode,
    ) -> Result<TlsConnector, NetResultStatus> {
        let mut tls_config = StreamUtils::create_tls_config(tls_mode)?;
        match protocol {
            NetProtocol::Http | NetProtocol::Grpc => {
                tls_config.alpn_protocols = match http_protocol {
                    Some(protocol) => match protocol {
                        NetHttpProtocol::Http1 => vec![b"http/1.1".to_vec()],
                        NetHttpProtocol::Http2 => vec![b"h2".to_vec()],
                    },
                    None => vec![b"h2".to_vec(), b"http/1.1".to_vec()],
                };
            }
            _ => (),
        }
        Ok(TlsConnector::from(Arc::new(tls_config)))
    }
}

#[async_trait::async_trait]
pub trait ConnectStream: AsyncRead + AsyncWrite + Unpin + Send + Sync + Debug + 'static {
    async fn connect(config: &NetConfig) -> Result<Self, NetResultStatus>
    where
        Self: Sized;
    fn alpn_protocol(&self) -> Option<&[u8]>;
}

#[async_trait::async_trait]
impl ConnectStream for TcpStream {
    async fn connect(config: &NetConfig) -> Result<Self, NetResultStatus> {
        StreamUtils::create_tcp_stream(&config.addr).await
    }

    fn alpn_protocol(&self) -> Option<&[u8]> {
        return None;
    }
}

#[async_trait::async_trait]
impl ConnectStream for arti_client::DataStream {
    async fn connect(config: &NetConfig) -> Result<Self, NetResultStatus> {
        StreamUtils::create_data_stream(config).await
    }
    fn alpn_protocol(&self) -> Option<&[u8]> {
        return None;
    }
}

#[async_trait::async_trait]
impl<T> ConnectStream for TlsStream<T>
where
    T: ConnectStream + AsyncReadWrite,
{
    async fn connect(config: &NetConfig) -> Result<Self, NetResultStatus> {
        let base_stream = T::connect(config).await?;
        StreamUtils::create_tls_stream(
            &config.addr,
            base_stream,
            &config.protocol,
            &config.http.protocol,
            &config.tls_mode,
        )
        .await
    }
    fn alpn_protocol(&self) -> Option<&[u8]> {
        return self.get_ref().1.alpn_protocol();
    }
}
pub type BoxedStream = Box<dyn ConnectStream>;
