use log::{Level, LevelFilter, Metadata, Record, debug, error};
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, Once, RwLock,
        atomic::{AtomicU32, Ordering},
    },
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

static TRANSPORTER_INSTANCES: Lazy<RwLock<HashMap<u32, Arc<DartTransporter>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));
// Atomic counter for unique IDs
static NEXT_INSTANCE_ID: AtomicU32 = AtomicU32::new(257);
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

static LOGGER_INIT: Once = Once::new();

fn init_logger_once() {
    LOGGER_INIT.call_once(|| {
        if log::set_logger(&LOGGER).is_ok() {
            log::set_max_level(LevelFilter::Debug);
            debug!("logging start");
        }
    });
}

static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().unwrap());

pub type DartCallbackC = extern "C" fn(response: *const NetResponseC);
struct TransporterEntry {
    transport: Box<dyn Transport + Send + Sync>,
}
pub struct DartTransporter {
    callback: Arc<RwLock<Option<DartCallbackC>>>,
    transports: Mutex<HashMap<u32, Arc<TransporterEntry>>>,
    next_id: Mutex<u32>,
    instance_id: u32,
}

impl DartTransporter {
    pub fn new(callback: DartCallbackC, instance_id: u32) -> Self {
        Self {
            callback: Arc::new(RwLock::new(Some(callback))),
            transports: Mutex::new(HashMap::new()),
            next_id: Mutex::new(258),
            instance_id,
        }
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
        let callback = Arc::clone(&self.callback);
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

            if let Some(cb) = *callback
                .read()
                .map_err(|_| NetResultStatus::InternalError)?
            {
                cb(ptr);
            }
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

        let transport_id = {
            let mut id_guard = self
                .next_id
                .lock()
                .map_err(|_| NetResultStatus::InternalError)?;
            let id = *id_guard;
            *id_guard = id_guard.checked_add(1).unwrap_or(258);
            id
        };
        debug!(
            "Transport created. instance: {:#?} protocol: {:#?}, url: {:#?}, id: {:#?} mode: {:#?}",
            self.instance_id,
            config.protocol,
            config.url.split('?').next().unwrap_or(&config.url),
            transport_id,
            config.mode,
        );
        let callback = Arc::clone(&self.callback);
        let rust_callback: DartCallback = Arc::new(move |response: NetResponseKind| {
            let response = NetResponse {
                request_id: 0,
                response,
                transport_id: transport_id,
            };
            let response_c = response.to_c();
            let boxed = Box::new(response_c);
            let ptr: *const NetResponseC = Box::into_raw(boxed);
            let g = callback.read();
            match g {
                Ok(e) => match *e {
                    Some(cb) => cb(ptr),
                    None => {}
                },
                Err(_) => {}
            };
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
        self.transports
            .lock()
            .map_err(|_| NetResultStatus::InternalError)?
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
            let guard = self
                .transports
                .lock()
                .map_err(|_| NetResultStatus::InternalError)?;
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
        let callback = Arc::clone(&self.callback);
        println!(
            "new request intance: {:#?} id: {:#?} transport: {:#?}",
            self.instance_id, request_id, id
        );
        // spawn async task on your static runtime
        RUNTIME.spawn(async move {
            let result = timeout(
                Duration::from_secs(request.timeout as u64),
                transport_arc.transport.do_request(request),
            )
            .await;
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
            let g = callback.read();

            match g {
                Ok(e) => match *e {
                    Some(cb) => {
                        debug!("cb called send");
                        cb(ptr);
                        println!("response post to dart");
                    }
                    None => {
                        debug!("called callback after remove.")
                    }
                },
                Err(_) => {
                    error!("read lock failed.")
                }
            };
        });

        Ok(())
    }

    pub fn close(&self, transport_id: u32) -> Result<(), NetResultStatus> {
        // Step 1: Remove the transport from the global map
        let transport_arc = {
            let mut guard = self
                .transports
                .lock()
                .map_err(|_| NetResultStatus::InternalError)?;
            if let Some(entry) = guard.remove(&transport_id) {
                entry
            } else {
                return Err(NetResultStatus::TransportNotFound);
            }
        };
        let callback = Arc::clone(&self.callback);
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
            let g = callback.read();
            match g {
                Ok(e) => match *e {
                    Some(cb) => cb(ptr),
                    None => {
                        debug!("called callback after remove.")
                    }
                },
                Err(_) => {
                    error!("read lock failed.")
                }
            };
        });

        Ok(())
    }

    /// Close all transports, ignoring callback results
    pub fn close_all(&self) -> Result<(), NetResultStatus> {
        // Step 1: Set callback to None
        self.callback
            .write()
            .map_err(|_| NetResultStatus::InternalError)?
            .take();

        // Step 2: Take all transports
        let transports: Vec<Arc<TransporterEntry>> = {
            let mut guard = self
                .transports
                .lock()
                .map_err(|_| NetResultStatus::InternalError)?;
            let all: Vec<Arc<TransporterEntry>> = guard.drain().map(|(_, t)| t).collect();
            all
        };

        // Step 3: Spawn async tasks to close each transport
        for transport_arc in transports {
            RUNTIME.spawn(async move {
                let _ = transport_arc.transport.close().await;
                // No callback called since we took it above
            });
        }
        Ok(())
    }
}

// Helper function to get transporter by ID
fn get_transporter_by_id(id: u32) -> Result<Arc<DartTransporter>, u8> {
    let guard = TRANSPORTER_INSTANCES
        .read()
        .map_err(|_| NetResultStatus::InternalError as u8)?;

    guard
        .get(&id)
        .cloned()
        .ok_or(NetResultStatus::InstanceDoesNotExist as u8)
}

#[unsafe(no_mangle)]
pub extern "C" fn dart_transporter_create(id: u32, config: *const NetConfigRequestC) -> u32 {
    match get_transporter_by_id(id) {
        Ok(transporter) => match transporter.create_transporter(config) {
            Ok(transport_id) => transport_id,
            Err(e) => e as u32,
        },
        Err(status) => status as u32,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn dart_transporter_send(id: u32, request: *const NetRequestC) -> u8 {
    match get_transporter_by_id(id) {
        Ok(transporter) => match transporter.send_request(request) {
            Ok(_) => NetResultStatus::OK as u8,
            Err(e) => e as u8,
        },
        Err(status) => status,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn dart_update_config(id: u32, request: *const NetRequestC) -> u8 {
    match get_transporter_by_id(id) {
        Ok(transporter) => match transporter.update_config(request) {
            Ok(_) => NetResultStatus::OK as u8,
            Err(e) => e as u8,
        },
        Err(status) => status,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dart_transporter_close(id: u32, transport_id: u32) -> u8 {
    match get_transporter_by_id(id) {
        Ok(transporter) => match transporter.close(transport_id) {
            Ok(_) => NetResultStatus::OK as u8,
            Err(e) => e as u8,
        },
        Err(status) => status,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dart_transporter_close_instance(id: u32) -> u8 {
    // Step 1 — remove transporter from global map
    let transporter = {
        let mut guard = TRANSPORTER_INSTANCES
            .write()
            .unwrap_or_else(|e| e.into_inner());
        guard.remove(&id)
    };

    // Step 2 — if not found → error
    let Some(t) = transporter else {
        return NetResultStatus::InstanceDoesNotExist as u8;
    };

    let _ = t.close_all();

    NetResultStatus::OK as u8
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dart_transporter_free_response(response: *const NetResponseC) -> u8 {
    if response.is_null() {
        return NetResultStatus::InternalError as u8;
    }
    unsafe { (*response).free_memory() };
    NetResultStatus::OK as u8
}

#[unsafe(no_mangle)]
pub extern "C" fn dart_transporter_create_instance(callback: DartCallbackC, debug: bool) -> u32 {
    // Initialize logger if debug is true
    if debug {
        let _ = init_logger_once();
    }
    // Generate unique ID >= 257
    let instance_id = NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed);
    // Create new DartTransporter instance
    let transporter = Arc::new(DartTransporter::new(callback, instance_id));

    // Store instance in global map
    let mut guard = match TRANSPORTER_INSTANCES.write() {
        Ok(g) => g,
        Err(_) => return NetResultStatus::InternalError as u32, // use u32 status code
    };
    guard.insert(instance_id, transporter);
    // Return the new transport ID
    instance_id
}
