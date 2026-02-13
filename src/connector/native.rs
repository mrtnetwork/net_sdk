use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{runtime::Runtime, time::timeout};

use crate::{
    transport::{
        Transport,
        native::{grpc::GrpcTransport, http::HttpTransport, socket::SocketTransport},
    },
    types::{
        DartCallback,
        config::{NetRequestConfig, NetRequestConfigC},
        error::NetResultStatus,
        request::{NetRequest, NetRequestC},
        response::{NetResponse, NetResponseC, NetResponseKind},
    },
};

pub fn init_logging() {
    // android_logger::init_once(Config::default().with_max_level(LevelFilter::Trace));
}

static TRANSPORTERS: Lazy<Mutex<HashMap<u32, Arc<TransporterEntry>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().unwrap());

static GLOBAL_TRANSPORTER: Lazy<Mutex<Option<DartTransporter>>> = Lazy::new(|| Mutex::new(None));
static mut NEXT_TRANSPORTER_ID: u32 = 257;
pub type DartCallbackC = extern "C" fn(response: *const NetResponseC);
struct TransporterEntry {
    transport: Box<dyn Transport>,
}
pub struct DartTransporter {
    callback: DartCallbackC,
}

impl DartTransporter {
    pub fn new(callback: DartCallbackC) -> Self {
        Self { callback }
    }
    pub fn create_transporter(
        &self,
        config: *const NetRequestConfigC,
    ) -> Result<u32, NetResultStatus> {
        if config.is_null() {
            return Err(NetResultStatus::InvalidConfigParameters);
        }
        let cfg: &NetRequestConfigC = unsafe {
            assert!(!config.is_null());
            &*config
        };
        let config = NetRequestConfig::try_from(cfg)?;
        let transport_id = unsafe {
            let id = NEXT_TRANSPORTER_ID;
            NEXT_TRANSPORTER_ID += 1;
            id
        };
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
            // Close the transport (async)
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

// struct TransporterEntry {
//     transport: Arc<Box<dyn Transport>>,
// }
#[unsafe(no_mangle)]
pub extern "C" fn dart_transporter_init(callback: DartCallbackC) -> u8 {
    init_logging();
    let mut guard = match GLOBAL_TRANSPORTER.lock() {
        Ok(g) => g,
        Err(_) => return NetResultStatus::InitializationFailed as u8,
    };
    if guard.is_some() {
        return NetResultStatus::AlreadyInitialized as u8;
    }

    *guard = Some(DartTransporter::new(callback));
    NetResultStatus::OK as u8
}
#[unsafe(no_mangle)]
pub extern "C" fn dart_transporter_create(config: *const NetRequestConfigC) -> u32 {
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
        return NetResultStatus::InvalidResponse as u8;
    }
    unsafe { (*response).free_memory() };
    NetResultStatus::OK as u8
}
