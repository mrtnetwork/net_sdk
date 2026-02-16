use std::collections::HashMap;
use std::sync::Arc;

use futures::FutureExt;
use futures::future::{Either, select};
use gloo_timers::future::TimeoutFuture;
use log::{Level, LevelFilter, Metadata, Record, debug};
use parking_lot::Mutex;
use wasm_bindgen::prelude::*;

use crate::types::config::NetConfigRequestWasm;
use crate::types::request::NetRequestWasm;
use crate::types::response::NetResponseWasm;
use crate::{
    transport::wasm::{
        Transport, grpc::GrpcTransport, http::HttpTransport, socket::SocketTransport,
    },
    types::{
        DartCallback,
        config::NetConfigRequest,
        error::NetResultStatus,
        request::NetRequest,
        response::{NetResponse, NetResponseKind},
    },
};

#[wasm_bindgen]
extern "C" {
    fn debug_print(s: String) -> bool;
}

struct SimpleLogger;

static LOGGER: SimpleLogger = SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            web_sys::console::log_1(&JsValue::from_str(&format!(
                "[{}] [{}] {}",
                record.level(),
                record.target(),
                record.args()
            )));
            // debug_print();
        }
    }

    fn flush(&self) {}
}

fn init_logger() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(LevelFilter::Debug);
}

#[wasm_bindgen(start)]
pub fn start() {
    init_logger();
}
struct TransporterEntry {
    transport: Box<dyn Transport>,
}

// static GLOBAL_MUX: Lazy<Mutex<Arc<js_sys::Function>>> = Lazy::new(|| Mutex::new(None));

#[wasm_bindgen]
pub struct DartTransporter {
    callback: Arc<js_sys::Function>,
    transports: Arc<Mutex<HashMap<u32, Arc<TransporterEntry>>>>,
    next_id: Arc<Mutex<u32>>,
}

#[wasm_bindgen]
impl DartTransporter {
    #[wasm_bindgen]
    pub fn version(&self) -> u32 {
        1
    }
    #[wasm_bindgen]
    pub fn create(callback: js_sys::Function) -> DartTransporter {
        DartTransporter::new(callback)
    }

    #[wasm_bindgen(constructor)]
    pub fn new(callback: js_sys::Function) -> DartTransporter {
        DartTransporter {
            callback: Arc::new(callback.clone()),
            transports: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(257)),
        }
    }
    pub fn create_transporter(&self, config: NetConfigRequestWasm) -> Result<u32, NetResultStatus> {
        let config: NetConfigRequest = config.to_config()?;
        let mut id_guard = self.next_id.lock();
        let transport_id = *id_guard;
        *id_guard += 1;
        drop(id_guard);

        let callback = self.callback.clone();
        let rust_callback: DartCallback = Arc::new(move |resp: NetResponseKind| {
            let response = NetResponse {
                request_id: 0,
                response: resp,
                transport_id,
            };

            let js_response = NetResponseWasm::from_native(response);
            let this = JsValue::NULL;
            let arg = JsValue::from(js_response);
            let _ = callback.call1(&this, &arg);
        });

        let transport: Box<dyn Transport> = match config.protocol {
            crate::types::config::NetProtocol::Http => {
                Box::new(HttpTransport::create(config, rust_callback, transport_id)?)
            }
            crate::types::config::NetProtocol::Grpc => {
                Box::new(GrpcTransport::create(config, rust_callback, transport_id)?)
            }
            crate::types::config::NetProtocol::WebSocket
            | crate::types::config::NetProtocol::Socket => Box::new(SocketTransport::create(
                config,
                rust_callback,
                transport_id,
            )?),
        };

        self.transports
            .lock()
            .insert(transport_id, Arc::new(TransporterEntry { transport }));

        Ok(transport_id)
    }

    #[wasm_bindgen]
    pub async fn send_request(
        &self,
        request: NetRequestWasm,
    ) -> Result<NetResponseWasm, NetResultStatus> {
        let request: NetRequest = request.to_native()?;
        let transports = self.transports.clone();
        let timeout_ms = request.timeout();
        let transport_id = request.transport_id();
        let id: u32 = request.id();

        // Get transport
        let transport_arc = {
            let guard = transports.lock();
            guard.get(&request.transport_id()).cloned()
        };

        let transport_arc = match transport_arc {
            Some(t) => t,
            None => return Err(NetResultStatus::TransportNotFound),
        };

        // request_future: some async future
        let request_future = transport_arc.transport.do_request(request).fuse();
        let timeout_future = TimeoutFuture::new(timeout_ms * 1000).fuse();

        // Pin both futures
        futures::pin_mut!(request_future, timeout_future);

        // Race the two
        let response = match select(request_future, timeout_future).await {
            Either::Left((res, _)) => match res {
                Ok(res) => res,
                Err(err) => NetResponseKind::ResponseError(err),
            }, // request completed first
            Either::Right((_, _)) => {
                NetResponseKind::ResponseError(NetResultStatus::RequestTimeout)
            }
        };

        let resp = NetResponse {
            transport_id: transport_id,
            response,
            request_id: id,
        };
        let js_response = NetResponseWasm::from_native(resp);
        // let arg: JsValue = JsValue::from(js_response);
        Ok(js_response)
    }

    #[wasm_bindgen]
    pub async fn close_transport(&self, transport_id: u32) -> NetResultStatus {
        debug!("close transport: {:#?}", transport_id);
        let transports = self.transports.lock().remove(&transport_id);
        let _ = match transports {
            Some(t) => t.transport.close().await,
            None => return NetResultStatus::TransportNotFound,
        };
        debug!("Transport closed: {:#?}", transport_id);
        NetResultStatus::OK
    }
}
