use crate::{
    client::wasm::{IClient, IHttpClient},
    types::{
        config::{NetConfig, NetHttpHeader},
        error::NetResultStatus,
        request::NetHttpRetryConfig,
        response::NetResponseHttp,
    },
    utils::buffer::{StreamBuffer, StreamEncoding},
};
use reqwest::{Client, RequestBuilder};
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
        retry: &NetHttpRetryConfig,
    ) -> Result<NetResponseHttp, NetResultStatus> {
        let mut attempt: u8 = 0;
        let req = self.build_requeest(url, method, body, headers)?;
        loop {
            let result = req
                .try_clone()
                .map_or(self.build_requeest(url, method, body, headers)?, |f| f)
                .send()
                .await;

            match result {
                Err(_) => {
                    if attempt >= retry.max_retries() {
                        return Err(NetResultStatus::ConnectionError);
                    }
                }

                Ok(resp) => {
                    let status_code = resp.status().as_u16();
                    let should_retry = retry.retry_status().contains(&status_code);

                    if should_retry && attempt < retry.max_retries() {
                        attempt += 1;
                        HttpClient::sleep_ms(retry.retry_delay()).await;
                        continue;
                    }

                    let is_success = (200..300).contains(&status_code);

                    let headers: Vec<NetHttpHeader> = resp
                        .headers()
                        .iter()
                        .map(|(k, v)| {
                            NetHttpHeader::new(
                                k.to_string(),
                                v.to_str().unwrap_or_default().to_string(),
                            )
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

                    return Ok(NetResponseHttp::new(status_code, body, headers, encoding));
                }
            }

            attempt += 1;
            HttpClient::sleep_ms(retry.retry_delay()).await;
        }
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
    async fn sleep_ms(ms: u32) {
        use wasm_bindgen_futures::JsFuture;

        let promise = js_sys::Promise::new(&mut |resolve, _| {
            web_sys::window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms as i32)
                .unwrap();
        });

        let _ = JsFuture::from(promise).await;
    }
    fn build_requeest(
        &self,
        url: &str,
        method: &str,
        body: Option<&[u8]>,
        headers: Option<&[NetHttpHeader]>,
    ) -> Result<RequestBuilder, NetResultStatus> {
        let method = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|_| NetResultStatus::InvalidRequestParameters)?;
        let mut req = self.sender.request(method.clone(), url.to_string());

        if let Some(headers) = headers {
            for h in headers {
                req = req.header(h.key(), h.value());
            }
        }
        if let Some(b) = body {
            req = req.body(b.to_vec());
        }
        Ok(req)
    }
}
