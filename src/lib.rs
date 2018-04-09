#[cfg(test)]
#[macro_use]
extern crate assert_matches;
#[macro_use]
extern crate error_chain;
extern crate fern;
extern crate futures;
#[macro_use]
extern crate hyper;
extern crate hyper_tls;
#[macro_use]
extern crate log;
extern crate native_tls;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate sha1;
extern crate time;
extern crate tokio_core;

use std::thread;
use std::sync::mpsc;
use std::os::raw::c_char;
use std::ffi::CString;
use error_chain::ChainedError;

mod pblite;
mod example;
mod hangouts;
mod decoder;
mod channel;
mod auth;
mod channel_parser;

#[no_mangle]
pub extern "C" fn say_hello() {
    println!("Hello, world!");
}

#[no_mangle]
pub extern "C" fn libhangups_client_create() -> *mut Client {
    let result = std::panic::catch_unwind(|| match Client::new() {
        Ok(client) => Box::into_raw(Box::new(client)),
        Err(e) => {
            error!("{}", e.display_chain());
            std::ptr::null_mut()
        }
    });
    match result {
        Ok(ptr) => ptr,
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn libhangups_client_receive(
    client_ptr: *const Client,
    timeout_millis: u64,
) -> *const c_char {
    let client = unsafe {
        assert!(!client_ptr.is_null());
        &*client_ptr
    };
    let timeout = std::time::Duration::from_millis(timeout_millis);
    // Use AssertUnwindSafe and assume the client is unusable if it panicked.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| client.receive(timeout)));
    match result {
        Ok(Some(received_string)) => CString::new(received_string).unwrap().into_raw(),
        Ok(None) => std::ptr::null_mut(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn libhangups_destroy_received(received: *mut c_char) {
    assert!(!received.is_null());
    unsafe {
        CString::from_raw(received);
    }
}

#[no_mangle]
pub extern "C" fn libhangups_client_destroy(client_ptr: *mut Client) {
    if client_ptr.is_null() {
        return;
    }
    unsafe {
        Box::from_raw(client_ptr);
    }
}

pub struct Client {
    rx: mpsc::Receiver<String>,
}
impl Client {
    fn new() -> auth::Result<Self> {
        setup_logger().unwrap();

        let cookies = auth::get_cookies()?;
        let mut channel = channel::Channel::new(cookies);

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            match channel.listen(&|state_update| {
                let state_update_json = serde_json::to_string(&state_update).unwrap();
                tx.send(state_update_json).unwrap();
            }) {
                Ok(_) => {}
                Err(e) => {
                    error!("{}", e.display_chain());
                }
            };
        });
        Ok(Self { rx: rx })
    }

    fn receive(&self, timeout: std::time::Duration) -> Option<String> {
        match self.rx.recv_timeout(timeout) {
            Ok(s) => Some(s),
            Err(mpsc::RecvTimeoutError::Timeout) => Some(String::from("{}")),
            Err(mpsc::RecvTimeoutError::Disconnected) => None,
        }
    }
}

// TODO: Replace this with a public function to configure a logging callback.
fn setup_logger() -> Result<(), fern::InitError> {
    println!("setup logging");
    let colors = fern::colors::ColoredLevelConfig::new();
    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{}][{}] {}",
                record.target(),
                colors.color(record.level()),
                message
            ))
        })
        .level(log::LevelFilter::Warn)
        .level_for("hangups", log::LevelFilter::Trace)
        .chain(std::io::stderr())
        .apply()?;
    Ok(())
}
