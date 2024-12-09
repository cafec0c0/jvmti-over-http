# JVMTI Over HTTP

A very small example of how JVMTI agents can be written in Rust.

When attached, this library will spawn a HTTP server, that will return JSON responses for a few URLs.

### How it works
To accomplish this, the library:
- Uses bindgen to generate Rust bindings to `jvmti.h`
- Create the `Agent_OnLoad`, `Agent_OnLoad` and `Agent_OnLoad`
- Does some horrible tricks to use mutable static variables to preserve `JavaVM` and `jvmtiEnv` pointers
- Uses more nasty mutable static variables to hold a reference to prevent Rust from dropping the tokio runtime and tiny http server.

## Attaching
Launch the JVM with the following argument (for the default port of 8001):
```
-agentpath:/path/to/library/libjvmti_over_http.so
```

Or, to specify the port, use the following argument:
```
-agentpath:/path/to/library/libjvmti_over_http.so=port=<port>
```

## Endpoints
This example currently supports two endpoints:
- `/VirtualMachine/Version` - this actually returns the JVMTI Version
- `/Class/GetLoadedClasses` - this returns the signature and generic signature of all loaded classes