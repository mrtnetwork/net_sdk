use crate::{
    client::wasm::{IClient, IHttpClient},
    types::{
        config::{NetConfig, NetHttpHeader},
        error::NetResultStatus,
        response::NetResponseHttp,
    },
    utils::buffer::{StreamBuffer, StreamEncoding},
};
use log::info;
use reqwest::Client;
use std::sync::Arc;

pub struct HttpClient {
    sender: Arc<Client>,
    config: NetConfig,
}
#[async_trait::async_trait(?Send)]
impl IClient for HttpClient {
    async fn connect(&self) -> Result<(), NetResultStatus> {
        Ok(())
    }

    fn get_config(&self) -> &NetConfig {
        &self.config
    }
}

#[async_trait::async_trait(?Send)]
impl IHttpClient for HttpClient {
    async fn send(
        &self,
        url: &str,
        method: &str,
        body: Option<&[u8]>,
        headers: Option<&[NetHttpHeader]>,
        encoding: StreamEncoding,
    ) -> Result<NetResponseHttp, NetResultStatus> {
        let method = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|_| NetResultStatus::InvalidRequestParameters)?;
        let mut req = self.sender.request(method, url.to_string());

        if let Some(headers) = headers {
            for h in headers {
                req = req.header(h.key(), h.value());
            }
        }

        if let Some(b) = body {
            req = req.body(b.to_vec());
        }
        let resp = req.send().await.map_err(|e| {
            info!("error data {:#?}", e.is_status());
            NetResultStatus::ConnectionError
        })?;
        let status_code = resp.status().as_u16();
        let is_success = (200..300).contains(&status_code);

        let headers: Vec<NetHttpHeader> = resp
            .headers()
            .iter()
            .map(|(k, v)| {
                NetHttpHeader::new(k.to_string(), v.to_str().unwrap_or_default().to_string())
            })
            .collect();

        let bytes = resp
            .bytes()
            .await
            .map_err(|_| NetResultStatus::ConnectionError)?;
        let (body, encoding) = if is_success {
            StreamBuffer::try_current_buffer(bytes.to_vec(), encoding)
        } else {
            (bytes.to_vec(), StreamEncoding::Raw)
        };

        Ok(NetResponseHttp::new(status_code, body, headers, encoding))
    }

    async fn close(&self) {}
}

impl HttpClient {
    pub fn new(config: NetConfig) -> Result<Self, NetResultStatus> {
        let client = Client::new();
        Ok(Self {
            sender: Arc::new(client),
            config,
        })
    }
}
