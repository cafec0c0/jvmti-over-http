#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(static_mut_refs)]
include!(concat!(env!("OUT_DIR"), "/jvmti.rs"));

use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::{c_char, c_void};
use std::ptr::{null_mut};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use ascii::AsciiString;
use c_vec::CVec;
use tiny_http::{Header, Request, Response, Server};
use tokio::runtime::Runtime;
use serde::Serialize;

struct JdwpServer {
    server: Option<Arc<Mutex<Server>>>,
    runtime: Option<Arc<Mutex<Runtime>>>,
    env: Option<Arc<Mutex<*mut jvmtiEnv>>>,
    vm: Option<Arc<Mutex<*mut JavaVM>>>
}

static mut jdwp_server: JdwpServer = JdwpServer {
    server: None,
    runtime: None,
    env: None,
    vm: None
};

pub fn parse_options(options: *mut c_char) -> HashMap<String, String> {
    let mut ops = HashMap::new();

    unsafe {
        if !options.is_null() {
            let c_str = CStr::from_ptr(options);
            let r_str = c_str.to_str().unwrap();
            r_str
                .split(",")
                .for_each(|pair| match pair.split_once("=") {
                    None => {
                        ops.insert(pair.to_string(), "".to_string());
                    }
                    Some((k, v)) => {
                        ops.insert(k.to_string(), v.to_string());
                    }
                });
        }
    }

    println!("Found {} options:", ops.len());
    for (k, v) in &ops {
        println!("  {}: {}", k, v);
    }

    ops
}

pub fn get_jvmti_env(vm: *mut JavaVM, version: jint) -> Result<*mut jvmtiEnv, String> {
    let env: *mut jvmtiEnv = null_mut();
    let ptr_to_env: *const *mut jvmtiEnv = &env;
    let err;

    unsafe {
        err = (*(*vm)).GetEnv.unwrap()(vm, ptr_to_env as *mut *mut c_void, version);
    }

    match err as u32 {
        JNI_OK => Ok(env),
        err => Err(String::from(format!("Unable to get environment: {}", err))),
    }
}


#[derive(Serialize)]
struct UnsupportedResponse<'a> {
    endpoint: &'a str,
    message: &'static str
}

#[derive(Serialize)]
struct VersionResponse {
    jvmti_major: jint,
    jvmti_minor: jint,
    jvmti_micro: jint,
}

#[derive(Serialize)]
struct ClassSignatures {
    signature: String,
    generic: Option<String>,
}

#[derive(Serialize)]
struct LoadedClassesResponse {
    classes: Vec<ClassSignatures>,
}

fn handle_version_command(request: Request) {
    unsafe {
        let env = *jdwp_server.env.as_ref().unwrap().lock().unwrap();

        let vm = *jdwp_server.vm.as_ref().unwrap().lock().unwrap();

        let env_ptr: *mut jvmtiEnv = null_mut();
        let ptr_to_env_ptr: *const *mut jvmtiEnv = &env_ptr;

        (*(*vm)).AttachCurrentThread.unwrap()(vm, ptr_to_env_ptr as *mut *mut c_void, null_mut() as *mut c_void);

        let mut version: jint = 0;
        (*(*env)).GetVersionNumber.unwrap()(env, &mut version as *mut jint);

        let version_response = VersionResponse {
            jvmti_major: (version & (JVMTI_VERSION_MASK_MAJOR as jint)) >> (JVMTI_VERSION_SHIFT_MAJOR),
            jvmti_minor: (version & (JVMTI_VERSION_MASK_MINOR as jint)) >> (JVMTI_VERSION_SHIFT_MINOR),
            jvmti_micro: (version & (JVMTI_VERSION_MASK_MICRO as jint)) >> (JVMTI_VERSION_SHIFT_MICRO),
        };

        let response = Response::from_string(serde_json::to_string(&version_response).unwrap());
        let response = response.with_header(Header {
           field: "Content-Type".parse().unwrap(),
            value: AsciiString::from_str("application/json").unwrap()
        });
        let _ = request.respond(response);
    }
}

fn handle_loaded_classes_command(request: Request) {
    unsafe {
        let env = *jdwp_server.env.as_ref().unwrap().lock().unwrap();

        let vm = *jdwp_server.vm.as_ref().unwrap().lock().unwrap();

        let env_ptr: *mut jvmtiEnv = null_mut();
        let ptr_to_env_ptr: *const *mut jvmtiEnv = &env_ptr;
        (*(*vm)).AttachCurrentThread.unwrap()(vm, ptr_to_env_ptr as *mut *mut c_void, null_mut() as *mut c_void);

        let mut class_count: jint = 0;
        let mut class_ptr: *mut jclass = null_mut();
        (*(*env)).GetLoadedClasses.unwrap()(env, &mut class_count as *mut jint, &mut class_ptr);

        let v = CVec::new(class_ptr, class_count as usize);

        let classes_vec = v.iter()
            .map(|klass| {
                let mut sig: *mut c_char = null_mut();
                let mut gen: *mut c_char = null_mut();
                (*(*env)).GetClassSignature.unwrap()(env, *klass, &mut sig, &mut gen);

                ClassSignatures {
                    signature: CStr::from_ptr(sig).to_str().unwrap().to_string(),
                    generic: if !gen.is_null() { Some(CStr::from_ptr(gen).to_str().unwrap().to_string()) } else { None },
                }
            })
            .collect::<Vec<ClassSignatures>>();

        let classes_response = LoadedClassesResponse {
          classes: classes_vec
        };

        let response = Response::from_string(serde_json::to_string(&classes_response).unwrap());
        let response = response.with_header(Header {
            field: "Content-Type".parse().unwrap(),
            value: AsciiString::from_str("application/json").unwrap()
        });
        let _ = request.respond(response);
    }
}

fn handle_unknown_command(request: Request) {
    let message = UnsupportedResponse {
        endpoint: request.url(),
        message: "The requested endpoint is not mapped to a JVMTI command"
    };
    let response = Response::from_string(serde_json::to_string(&message).unwrap());
    let response = response.with_header(Header {
        field: "Content-Type".parse().unwrap(),
        value: AsciiString::from_str("application/json").unwrap()
    });
    let _ = request.respond(response);
}

fn init(vm: *mut JavaVM, options: *mut c_char) {
    let options = parse_options(options);
    let port = u16::from_str(options.get("port").unwrap_or(&String::from("8001"))).unwrap();

    let server = Arc::new(Mutex::new(Server::http(format!("127.0.0.1:{}", port)).unwrap()));
    let rt = Arc::new(Mutex::new(Runtime::new().unwrap()));

    unsafe {
        // We need to keep a reference to these so that when this method goes out of scope,
        // they don't get freed.
        jdwp_server.runtime = Some(Arc::clone(&rt));
        jdwp_server.server = Some(Arc::clone(&server));


        let env = get_jvmti_env(vm, JVMTI_VERSION_21 as jint).unwrap();
        let env = Arc::new(Mutex::new(env));
        jdwp_server.env = Some(Arc::clone(&env));

        let vm = Arc::new(Mutex::new(vm));
        jdwp_server.vm = Some(Arc::clone(&vm));

        rt.lock().unwrap().spawn(async move {
            // (*(*vm)).AttachCurrentThread.unwrap()(vm, env2, null() as *mut c_void);
            for request in server.lock().unwrap().incoming_requests() {
                match request.url() {
                    "/VirtualMachine/Version" => handle_version_command(request),
                    "/Class/GetLoadedClasses" => handle_loaded_classes_command(request),
                    _ => handle_unknown_command(request),
                }
            }
        });


        println!("Internal Server started on port {}", port);
    }
}

#[no_mangle]
pub fn Agent_OnLoad(vm: *mut JavaVM, options: *mut c_char, _reserved: *mut c_void) -> jint {
    println!("**************** Started Loading ****************");
    init(vm, options);
    println!("**************** Finished Loading ****************");
    0
}

#[no_mangle]
pub fn Agent_OnAttach(vm: *mut JavaVM, options: *mut c_char, _reserved: *mut c_void) -> jint {
    println!("**************** Started Attach ****************");
    init(vm, options);
    println!("**************** Finished Attach ****************");
    0
}

#[no_mangle]
pub fn Agent_OnUnload(_vm: *mut JavaVM) {
    println!("Unloaded");
}

