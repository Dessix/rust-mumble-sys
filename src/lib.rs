#![feature(nll)]
#![allow(dead_code)]

use std::ffi::CString;
use std::os::raw;
use parking_lot::Mutex;
// use std::mem::MaybeUninit;

mod mumble;
pub mod traits;

pub use crate::mumble::root as types;
use types as m;

struct PluginFFIMetadata {
    name: CString,
    author: CString,
    description: CString,
    version: m::Version,
}

struct PluginHolder {
    metadata: PluginFFIMetadata,
    plugin: Box<dyn traits::MumblePlugin>,
    updater: Option<Box<dyn traits::MumblePluginUpdater>>,
    id: Option<m::plugin_id_t>,
    api: Option<m::MumbleAPI>,
}
//unsafe impl std::marker::Send for PluginHolder { }


static mut PLUGIN: Mutex<Option<PluginHolder>> = Mutex::new(None);

fn set_plugin_or_panic(plugin: PluginHolder) {
    let replaced: Option<_> = unsafe {
        let mut locked = PLUGIN.lock();
        locked.replace(plugin)
    };
    if replaced.is_some() {
        panic!("Duplicate plugin registrations occurred; bailing out.")
    }
}

fn lock_plugin<'a>() -> parking_lot::MappedMutexGuard<'a, PluginHolder> {
    use parking_lot::MutexGuard;
    let locked = (unsafe {
        &mut PLUGIN
    }).lock();
    if locked.is_none() {
        panic!("Plugin not initialized!");
    }
    MutexGuard::map(locked, |contents| {
        contents.as_mut().unwrap()
    })
}

fn run_with_plugin<T>(cb: fn(&mut PluginHolder) -> T) -> T {
    let mut holder = lock_plugin();
    cb(&mut holder)
}



pub fn register_plugin(
    name: &str,
    author: &str,
    description: &str,
    version: m::Version,
    plugin: Box<dyn traits::MumblePlugin>,
    updater: Option<Box<dyn traits::MumblePluginUpdater>>
) {
    let plugin = PluginHolder {
        metadata: PluginFFIMetadata {
            name: CString::new(name)
                .expect("Name must be representable as a CString"),
            author: CString::new(author)
                .expect("Author must be representable as a CString"),
            description: CString::new(description)
                .expect("Description must be representable as a CString"),
            version,
        },
        plugin,
        updater,
        id: None,
        api: None,
    };
    set_plugin_or_panic(plugin)
}


#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_registerAPIFunctions(api: m::MumbleAPI) {
    lock_plugin().api = Some(api);
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_registerPluginID(id: m::plugin_id_t) {
    lock_plugin().id = Some(id);
}

#[no_mangle]
pub extern fn mumble_init() -> m::mumble_error_t {
    lock_plugin().plugin.init()
}

#[no_mangle]
pub extern fn mumble_shutdown() {
    lock_plugin().plugin.shutdown()
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_getName() -> *const raw::c_char {
    lock_plugin().metadata.name.as_ptr()
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_getAPIVersion() -> m::Version {
    lock_plugin().metadata.version
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onUserTalkingStateChanged(
    conn: m::mumble_connection_t,
    user: m::mumble_userid_t,
    talking_state: m::talking_state_t,
) {
    let mut holder = lock_plugin();
    holder.plugin.on_user_talking_state_changed(conn, user, talking_state);
}

