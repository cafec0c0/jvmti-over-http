#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
include!(concat!(env!("OUT_DIR"), "/jvmti.rs"));

use std::os::raw::{c_char, c_void};
use std::sync::{Arc, Mutex};
use tiny_http::Server;
use tokio::runtime::Runtime;

struct JdwpServer {
    server: Option<Arc<Mutex<Server>>>,
    runtime: Option<Arc<Mutex<Runtime>>>
}

static mut jdwp_server: JdwpServer = JdwpServer {
    server: None,
    runtime: None
};

pub fn init() {
    let server = Arc::new(Mutex::new(Server::http("127.0.0.1:8082").unwrap()));
    let rt = Arc::new(Mutex::new(Runtime::new().unwrap()));
    unsafe {
        jdwp_server.runtime = Some(Arc::clone(&rt));
        jdwp_server.server = Some(Arc::clone(&server));

        let mut cloned_server = Arc::clone(&server);
        let mut cloned_rt = Arc::clone(&rt);
        println!("Before spawn");
        cloned_rt.lock().unwrap().spawn(async move {
            println!("Before loop");
            for request in cloned_server.lock().unwrap().incoming_requests() {
                println!("Request! {}", request.method());
            }
            println!("After loop")
        });
        println!("after spawn");
    }
}

#[no_mangle]
pub fn Agent_OnLoad(vm: *mut JavaVM, options: *mut c_char, _reserved: *mut c_void) -> jint {
    println!("**************** Started Loading ****************");
    let _ = init();
    println!("**************** Finished Loading ****************");
    0
}

#[no_mangle]
pub fn Agent_OnAttach(vm: *mut JavaVM, options: *mut c_char, _reserved: *mut c_void) -> jint {
    println!("**************** Started Attach ****************");
    let _ = init();
    println!("**************** Finished Attach ****************");
    0
}

#[no_mangle]
pub fn Agent_OnUnload(_vm: *mut JavaVM) {
    println!("Unloaded");
}

