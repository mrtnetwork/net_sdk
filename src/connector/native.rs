use log::{Level, LevelFilter, Metadata, Record, SetLoggerError, debug};
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{runtime::Runtime, time::timeout};

use crate::{
    stream,
    transport::native::{
        Transport, grpc::GrpcTransport, http::HttpTransport, socket::SocketTransport,
    },
    types::{
        DartCallback,
        config::NetConfigRequest,
        error::NetResultStatus,
        native::{
            c_tyes::{NetConfigRequestC, NetRequestC, NetResponseC},
            request::NetRequest,
        },
        response::{NetResponse, NetResponseKind},
    },
};

struct SimpleLogger;

static LOGGER: SimpleLogger = SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        // Only enable debug logs for your crate
        // Replace "my_crate" with your Cargo.toml package name
        let is_my_crate = metadata.target().starts_with("net_sdk");

        metadata.level() <= Level::Debug && is_my_crate
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!(
                "[{}] [{}] {}",
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

fn init_logger() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER)?;
    log::set_max_level(LevelFilter::Debug);
    debug!("logging start");
    Ok(())
}
static TRANSPORTERS: Lazy<Mutex<HashMap<u32, Arc<TransporterEntry>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().unwrap());

static GLOBAL_TRANSPORTER: Lazy<Mutex<Option<DartTransporter>>> = Lazy::new(|| Mutex::new(None));
static mut NEXT_TRANSPORTER_ID: u32 = 257;
pub type DartCallbackC = extern "C" fn(response: *const NetResponseC);
struct TransporterEntry {
    transport: Box<dyn Transport + Send + Sync>,
}
pub struct DartTransporter {
    callback: DartCallbackC,
}

impl DartTransporter {
    pub fn new(callback: DartCallbackC) -> Self {
        Self { callback }
    }
    pub fn update_config(&self, request: *const NetRequestC) -> Result<(), NetResultStatus> {
        if request.is_null() {
            return Err(NetResultStatus::InvalidRequestParameters);
        }
        let request: &NetRequestC = unsafe {
            assert!(!request.is_null());
            &*request
        };
        let request = unsafe { NetRequest::from_c(request) }?;
        let callback = self.callback.clone();
        RUNTIME.spawn(async move {
            let response = match request.kind {
                crate::types::native::request::NetRequestKind::InitTor(net_config_tor) => {
                    let init = stream::StreamUtils::init_tor_config(&net_config_tor).await;
                    match init {
                        Ok(_) => NetResponseKind::TorInited(true),
                        Err(e) => NetResponseKind::ResponseError(e),
                    }
                }
                crate::types::native::request::NetRequestKind::TorInited => {
                    let inited = stream::StreamUtils::tor_inited();
                    NetResponseKind::TorInited(inited)
                }
                _ => return Err(NetResultStatus::InvalidRequestParameters),
            };
            let response: NetResponse = NetResponse {
                request_id: request.id,
                response,
                transport_id: request.transport_id,
            };
            let response_c = response.to_c();
            let boxed = Box::new(response_c);
            let ptr: *const NetResponseC = Box::into_raw(boxed);
            (callback)(ptr);
            Ok(())
        });
        Ok(())
    }

    pub fn create_transporter(
        &self,
        config: *const NetConfigRequestC,
    ) -> Result<u32, NetResultStatus> {
        if config.is_null() {
            return Err(NetResultStatus::InvalidConfigParameters);
        }
        let cfg: &NetConfigRequestC = unsafe {
            assert!(!config.is_null());
            &*config
        };
        let config = NetConfigRequest::try_from(cfg)?;
        let transport_id = unsafe {
            let id = NEXT_TRANSPORTER_ID;
            NEXT_TRANSPORTER_ID += 1;
            id
        };
        debug!("Transport created: {:#?}", transport_id);
        let callback = self.callback.clone();
        let rust_callback: DartCallback = Arc::new(move |response: NetResponseKind| {
            let response = NetResponse {
                request_id: 0,
                response,
                transport_id: transport_id,
            };
            let response_c = response.to_c();
            let boxed = Box::new(response_c);
            let ptr: *const NetResponseC = Box::into_raw(boxed);
            (callback)(ptr);
        });
        let transport: Box<dyn Transport + Send + Sync> = match config.protocol {
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
        TRANSPORTERS
            .lock()
            .unwrap()
            .insert(transport_id, Arc::new(TransporterEntry { transport }));
        Ok(transport_id)
    }

    pub fn send_request(&self, request: *const NetRequestC) -> Result<(), NetResultStatus> {
        if request.is_null() {
            return Err(NetResultStatus::InvalidRequestParameters);
        }
        let request: &NetRequestC = unsafe {
            assert!(!request.is_null());
            &*request
        };
        let request = unsafe { NetRequest::from_c(request) }?;
        let transport_arc = {
            let guard = TRANSPORTERS.lock().unwrap();
            if let Some(entry) = guard.get(&(request.transport_id)) {
                Arc::clone(&entry)
            } else {
                return Err(NetResultStatus::TransportNotFound);
            }
        };
        let protocol = transport_arc.transport.get_config().protocol;
        let _ = request.to_protocol_config(protocol)?;

        let id = request.transport_id;
        let request_id = request.id;
        let callback = self.callback.clone();
        debug!("requst with timeout {:#?}", request.timeout);
        // spawn async task on your static runtime
        RUNTIME.spawn(async move {
            let result = timeout(
                Duration::from_secs(request.timeout as u64),
                transport_arc.transport.do_request(request),
            )
            .await;
            let guard = TRANSPORTERS.lock().unwrap();
            if let Some(_) = guard.get(&id) {
                let response = match result {
                    Ok(inner) => inner.map_or_else(|e| NetResponseKind::ResponseError(e), |e| e),
                    Err(_) => NetResponseKind::ResponseError(NetResultStatus::RequestTimeout),
                };

                let response = NetResponse {
                    transport_id: id,
                    response,
                    request_id,
                };

                let response_c = response.to_c();
                let boxed = Box::new(response_c);
                let ptr: *const NetResponseC = Box::into_raw(boxed);
                (callback)(ptr);
            }
        });

        Ok(())
    }

    pub fn close(&self, transport_id: u32) -> Result<(), NetResultStatus> {
        debug!("Close transport: {:#?}", transport_id);
        // Step 1: Remove the transport from the global map
        let transport_arc = {
            let mut guard = TRANSPORTERS.lock().unwrap();
            if let Some(entry) = guard.remove(&transport_id) {
                entry
            } else {
                return Err(NetResultStatus::TransportNotFound);
            }
        };
        let callback = self.callback.clone();
        // Step 2: Spawn async task to close transport
        RUNTIME.spawn(async move {
            let _ = transport_arc.transport.close().await;

            let response = NetResponse {
                transport_id,
                response: NetResponseKind::TransportClosed,
                request_id: 0,
            };
            let response_c = response.to_c();
            let boxed = Box::new(response_c);
            let ptr: *const NetResponseC = Box::into_raw(boxed);
            (callback)(ptr);
        });

        Ok(())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dart_transporter_init(callback: DartCallbackC, debug: bool) -> u8 {
    let mut guard = match GLOBAL_TRANSPORTER.lock() {
        Ok(g) => g,
        Err(_) => return NetResultStatus::InitializationFailed as u8,
    };
    if debug {
        let _ = init_logger();
    }
    if guard.is_none() {
        *guard = Some(DartTransporter::new(callback));
    }
    NetResultStatus::OK as u8
}
#[unsafe(no_mangle)]
pub extern "C" fn dart_transporter_create(config: *const NetConfigRequestC) -> u32 {
    let guard = GLOBAL_TRANSPORTER.lock().unwrap();
    match guard.as_ref() {
        Some(e) => {
            let create = e.create_transporter(config);
            match create {
                Ok(e) => e,
                Err(e) => e as u32,
            }
        }
        None => NetResultStatus::NotInitialized as u32,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn dart_transporter_send(request: *const NetRequestC) -> u8 {
    let guard = GLOBAL_TRANSPORTER.lock().unwrap();

    match guard.as_ref() {
        Some(e) => {
            let create = e.send_request(request);
            match create {
                Ok(_) => NetResultStatus::OK as u8,
                Err(e) => e as u8,
            }
        }
        None => NetResultStatus::NotInitialized as u8,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn dart_update_config(request: *const NetRequestC) -> u8 {
    let guard = GLOBAL_TRANSPORTER.lock().unwrap();

    match guard.as_ref() {
        Some(e) => {
            let create = e.update_config(request);
            match create {
                Ok(_) => NetResultStatus::OK as u8,
                Err(e) => e as u8,
            }
        }
        None => NetResultStatus::NotInitialized as u8,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn dart_transporter_close(transport_id: u32) -> u8 {
    let guard = GLOBAL_TRANSPORTER.lock().unwrap();
    match guard.as_ref() {
        Some(e) => {
            let create = e.close(transport_id);
            match create {
                Ok(_) => NetResultStatus::OK as u8,
                Err(e) => e as u8,
            }
        }
        None => NetResultStatus::NotInitialized as u8,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dart_transporter_free_response(response: *const NetResponseC) -> u8 {
    if response.is_null() {
        return NetResultStatus::InternalError as u8;
    }
    unsafe { (*response).free_memory() };
    NetResultStatus::OK as u8
}
