#![feature(nll)]
#![allow(dead_code)]

use std::ffi::CString;
use std::os::raw;
use parking_lot::Mutex;
use std::mem::MaybeUninit;

mod mumble;
pub mod traits;

pub use crate::mumble::root as types;
use types as m;

pub struct MumbleAPI {
    id: m::plugin_id_t,
    api: m::MumbleAPI,
}

impl MumbleAPI {

    pub fn get_active_server_connection(&mut self) -> m::mumble_connection_t {
        let mut conn_id = MaybeUninit::uninit();
        let f = self.api.getActiveServerConnection.unwrap();
        unsafe {
            f(self.id, conn_id.as_mut_ptr());
            conn_id.assume_init()
        }
    }

    pub fn get_local_user_id(&mut self, conn: m::mumble_connection_t) -> m::mumble_userid_t {
        let mut user_id = MaybeUninit::uninit();
        let f = self.api.getLocalUserID.unwrap();
        unsafe {
            f(self.id, conn,  user_id.as_mut_ptr());
            user_id.assume_init()
        }
    }
}

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
    raw_api: Option<m::MumbleAPI>,
}
//unsafe impl std::marker::Send for PluginHolder { }


static mut PLUGIN_REGISTRATION_CB: Mutex<Option<Box<dyn FnMut(RegistrationToken) -> ()>>> =
    Mutex::new(None);
static mut PLUGIN: Mutex<Option<PluginHolder>> = Mutex::new(None);

fn try_lock_plugin<'a>() -> Result<parking_lot::MappedMutexGuard<'a, PluginHolder>, String> {
    use parking_lot::MutexGuard;
    let mut locked = (unsafe {
        &mut PLUGIN
    }).lock();
    if locked.is_none() {
        let mut registration_cb = unsafe {
            PLUGIN_REGISTRATION_CB.lock()
        };

        if registration_cb.is_none() {
            return Err(String::from("Plugin not initialized and no registration callback is registered!"));
        } else {
            let rtok = RegistrationToken { _registration: &mut (*locked) };
            registration_cb.as_mut().unwrap()(rtok);
        };

        if locked.is_none() {
            return Err(String::from("Plugin not initialized after registration callback call!"));
        }
    }
    Ok(MutexGuard::map(locked, |contents| {
        contents.as_mut().unwrap()
    }))
}

fn lock_plugin<'a>() -> parking_lot::MappedMutexGuard<'a, PluginHolder> {
    match try_lock_plugin() {
        Ok(res) => res,
        Err(e) => panic!("{}", e)
    }
}

fn run_with_plugin<T>(cb: fn(&mut PluginHolder) -> T) -> T {
    let mut holder = lock_plugin();
    cb(&mut holder)
}

pub struct RegistrationToken<'a> {
    _registration: &'a mut Option<PluginHolder>,
}

pub fn set_registration_callback(
    cb: Box<dyn FnMut(RegistrationToken) -> ()>
) {
    unsafe {
        let mut locked = PLUGIN_REGISTRATION_CB.lock();
        locked.replace(cb)
    };
}

pub fn register_plugin(
    name: &str,
    author: &str,
    description: &str,
    version: m::Version,
    plugin: Box<dyn traits::MumblePlugin>,
    updater: Option<Box<dyn traits::MumblePluginUpdater>>,
    registration_token: RegistrationToken,
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
        raw_api: None,
    };
    let replaced: Option<_> = registration_token._registration.replace(plugin);
    if replaced.is_some() {
        panic!("Duplicate plugin registrations occurred; bailing out.")
    }
}


#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_registerAPIFunctions(api: m::MumbleAPI) {
    let mut holder = lock_plugin();
    holder.raw_api = Some(api);
    if let Some(id) = holder.id {
        holder.plugin.set_api(MumbleAPI { api, id });
    }
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_registerPluginID(id: m::plugin_id_t) {
    let mut holder = lock_plugin();
    holder.id = Some(id);
    if let Some(api) = holder.raw_api {
        holder.plugin.set_api(MumbleAPI { api, id });
    }
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

