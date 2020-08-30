#![feature(nll)]
#![allow(dead_code)]

use std::ffi::{CStr, CString};
use std::os::raw;
use parking_lot::Mutex;
use std::mem::MaybeUninit;

mod mumble;
pub mod traits;

pub use crate::mumble::root as types;
use types as m;
use std::ops::Deref;
use traits::CheckableId;

pub struct MumbleAPI {
    id: m::plugin_id_t,
    api: m::MumbleAPI,
}

pub struct Freeable<T> {
    pointer: *mut T,
    plugin_id: m::plugin_id_t,
    raw_api: m::MumbleAPI,
}

pub struct FreeableMaybeUninit<T> {
    uninit: MaybeUninit<*mut T>,
    freeable: Option<Freeable<T>>,
    plugin_id: m::plugin_id_t,
    raw_api: m::MumbleAPI,
}

impl MumbleAPI {
    fn freeable_uninit<T>(&self) -> FreeableMaybeUninit<T> {
        FreeableMaybeUninit {
            freeable: None,
            uninit: MaybeUninit::uninit(),
            plugin_id: self.id,
            raw_api: self.api,
        }
    }

    fn freeable_for<T>(&self, pointer: *mut T) -> Freeable<T> {
        Freeable::of(self.id, self.api, pointer)
    }

    pub fn get_active_server_connection(&self) -> m::mumble_connection_t {
        let mut conn_id = MaybeUninit::uninit();
        let f = self.api.getActiveServerConnection.unwrap();
        unsafe {
            f(self.id, conn_id.as_mut_ptr());
            conn_id.assume_init()
        }
    }

    pub fn is_connection_synchronized(&self, conn: m::mumble_connection_t) -> bool {
        let mut synchronized = MaybeUninit::uninit();
        let f = self.api.isConnectionSynchronized.unwrap();
        unsafe {
            f(self.id, conn, synchronized.as_mut_ptr());
            synchronized.assume_init()
        }
    }

    pub fn get_local_user_id(&mut self, conn: m::mumble_connection_t) -> m::mumble_userid_t {
        let mut user_id = MaybeUninit::uninit();
        let f = self.api.getLocalUserID.unwrap();
        unsafe {
            f(self.id, conn, user_id.as_mut_ptr());
            user_id.assume_init()
        }
    }

    pub fn get_user_name(
        &mut self,
        conn: m::mumble_connection_t,
        user_id: m::mumble_userid_t,
    ) -> String {
        let mut user_name_ref: FreeableMaybeUninit<raw::c_char> = self.freeable_uninit();
        let f = self.api.getUserName.unwrap();
        unsafe {
            f(self.id, conn, user_id, user_name_ref.as_mut_ptr());
            let name = CStr::from_ptr(user_name_ref.assume_init())
                .to_str().expect("Must be valid utf8").to_string();
            name
        }
    }

    pub fn get_user_hash(
        &mut self,
        conn: m::mumble_connection_t,
        user_id: m::mumble_userid_t,
    ) -> String {
        let mut user_hash_ref: FreeableMaybeUninit<raw::c_char> = self.freeable_uninit();
        let f = self.api.getUserHash.unwrap();
        unsafe {
            f(self.id, conn, user_id, user_hash_ref.as_mut_ptr());
            let name = CStr::from_ptr(user_hash_ref.assume_init())
                .to_str().expect("Must be valid utf8").to_string();
            name
        }
    }
}

impl<T> Freeable<T> {
    fn of(plugin_id: m::plugin_id_t, api: m::MumbleAPI, pointer: *mut T) -> Freeable<T> {
        println!("+{:?}", pointer);
        Freeable { plugin_id, raw_api: api, pointer }
    }
}

impl<T> Drop for Freeable<T> {
    fn drop(&mut self) {
        println!("-{:?}", self.pointer);
        let free_memory = self.raw_api.freeMemory.unwrap();
        let res = unsafe { free_memory(self.plugin_id, self.pointer.cast()) };
        assert_eq!(res, m::Mumble_ErrorCode::EC_OK, "free_memory must return OK");
    }
}

impl<T> Deref for Freeable<T> {
    type Target = *mut T;

    fn deref(&self) -> &Self::Target {
        &self.pointer
    }
}

impl<T> FreeableMaybeUninit<T> {
    pub fn as_mut_ptr(&mut self) -> *mut *mut T {
        if self.freeable.is_some() {
            self.freeable = None;
        }
        self.uninit.as_mut_ptr()
    }

    pub unsafe fn assume_init(&mut self) -> *mut T {
        let val = self.uninit.assume_init();
        if self.freeable.is_none() {
            self.freeable = Some(Freeable::of(
                self.plugin_id,
                self.raw_api,
                val));
        }
        val
    }
}

struct PluginFFIMetadata {
    name: CString,
    author: CString,
    description: CString,
    api_version: m::Version,
    version: m::Version,
}

struct PluginHolder {
    metadata: PluginFFIMetadata,
    plugin: Box<dyn traits::MumblePlugin>,
    updater: Option<Box<dyn traits::MumblePluginUpdater>>,
    id: Option<m::plugin_id_t>,
    raw_api: Option<m::MumbleAPI>,
}
impl PluginHolder {
    pub fn set_api(&mut self, api: m::MumbleAPI) {
        self.raw_api = Some(api);
        if let Some(id) = self.id {
            self.plugin.set_api(MumbleAPI { api, id });
        }
    }

    pub fn set_plugin_id(&mut self, id: m::plugin_id_t) {
        self.id = Some(id);
        if let Some(api) = self.raw_api {
            self.plugin.set_api(MumbleAPI { api, id });
        }
    }
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
    api_version: m::Version,
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
            api_version,
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

impl self::traits::CheckableId for m::mumble_channelid_t {
    fn check(self) -> Option<Self> {
        if (*self).is_negative() { None } else { Some(self) }
    }
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_registerAPIFunctions(api: m::MumbleAPI) {
    let mut holder = lock_plugin();
    holder.set_api(api);
}

#[no_mangle]
pub extern fn mumble_init(plugin_id: m::plugin_id_t) -> m::mumble_error_t {
    let mut holder = lock_plugin();
    holder.set_plugin_id(plugin_id);
    assert!(holder.id.is_some());
    assert!(holder.raw_api.is_some(), "RegisterAPIFunctions must have been called before init");
    holder.plugin.init()
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
    lock_plugin().metadata.api_version
}

// API not implemented: mumble_setMumbleInfo

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_getVersion() -> m::Version {
    lock_plugin().metadata.version
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_getAuthor() -> *const raw::c_char {
    lock_plugin().metadata.author.as_ptr()
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_getDescription() -> *const raw::c_char {
    lock_plugin().metadata.description.as_ptr()
}

// API not implemented: mumble_getFeatures
// API not implemented: mumble_deactivateFeatures

// API not implemented: mumble_initPositionalData
// API not implemented: mumble_fetchPositionalData
// API not implemented: mumble_shutdownPositionalData

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onServerConnected(conn: m::mumble_connection_t) {
    lock_plugin().plugin.on_server_connected(conn);
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onServerDisconnected(conn: m::mumble_connection_t) {
    lock_plugin().plugin.on_server_disconnected(conn);
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onServerSynchronized(conn: m::mumble_connection_t) {
    lock_plugin().plugin.on_server_synchronized(conn);
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onChannelEntered(
    conn: m::mumble_connection_t,
    user: m::mumble_userid_t,
    previous: m::mumble_channelid_t,
    current: m::mumble_channelid_t,
) {
    lock_plugin().plugin.on_channel_entered(conn, user, previous.check(), current.check());
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onChannelExited(
    conn: m::mumble_connection_t,
    user: m::mumble_userid_t,
    exited: m::mumble_channelid_t,
) {
    lock_plugin().plugin.on_channel_exited(conn, user, exited.check());
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onUserTalkingStateChanged(
    conn: m::mumble_connection_t,
    user: m::mumble_userid_t,
    talking_state: m::talking_state_t,
) {
    lock_plugin().plugin.on_user_talking_state_changed(conn, user, talking_state);
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onAudioInput(
    input_pcm: *mut raw::c_short,
    sample_count: u32,
    channel_count: u16,
    is_speech: bool,
) -> bool {
    let length = (sample_count as usize) * (channel_count as usize);
    // https://docs.rs/ndarray/0.13.1/ndarray/type.ArrayViewMut.html can be used for a nicer PCM API
    let pcm = unsafe {
        std::slice::from_raw_parts_mut::<i16>(input_pcm, length)
    };
    lock_plugin().plugin.on_audio_input(pcm, sample_count, channel_count, is_speech)
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onAudioSourceFetched(
    output_pcm: *mut f32,
    sample_count: u32,
    channel_count: u16,
    is_speech: bool,
    user_id: m::mumble_userid_t,
) -> bool {
    let length = (sample_count as usize) * (channel_count as usize);
    // https://docs.rs/ndarray/0.13.1/ndarray/type.ArrayViewMut.html can be used for a nicer PCM API
    let pcm = unsafe {
        std::slice::from_raw_parts_mut::<f32>(output_pcm, length)
    };
    let maybe_user_id = if is_speech { Some(user_id) } else { None };
    lock_plugin().plugin
        .on_audio_source_fetched(pcm, sample_count, channel_count, is_speech, maybe_user_id)
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onAudioOutputAboutToPlay(
    output_pcm: *mut f32,
    sample_count: u32,
    channel_count: u16,
) -> bool {
    let length = (sample_count as usize) * (channel_count as usize);
    // https://docs.rs/ndarray/0.13.1/ndarray/type.ArrayViewMut.html can be used for a nicer PCM API
    let pcm = unsafe {
        std::slice::from_raw_parts_mut::<f32>(output_pcm, length)
    };
    lock_plugin().plugin.on_audio_output_about_to_play(pcm, sample_count, channel_count)
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onReceiveData(
    conn: m::mumble_connection_t,
    sender: m::mumble_userid_t,
    data: *const raw::c_char,
    data_length: usize,
    data_id: *const raw::c_char,
) -> bool {
    // https://docs.rs/ndarray/0.13.1/ndarray/type.ArrayViewMut.html can be used for a nicer PCM API
    let data_id = unsafe {
        CStr::from_ptr(data_id).to_str()
            .expect("data_id must be a valid null-terminated string")
    };

    lock_plugin().plugin.on_receive_data(conn, sender, data_id, &|| {
        let data = data as *const u8;
        let data = unsafe {
            let bytes = std::slice::from_raw_parts(data, data_length as usize);
            CStr::from_bytes_with_nul(bytes)
                .expect("data must be a valid null-terminated string")
        };
        data.to_str().unwrap().to_string()
    })
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onUserAdded(
    conn: m::mumble_connection_t,
    user: m::mumble_userid_t,
) {
    lock_plugin().plugin.on_user_added(conn, user);
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onUserRemoved(
    conn: m::mumble_connection_t,
    user: m::mumble_userid_t,
) {
    lock_plugin().plugin.on_user_removed(conn, user);
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onChannelAdded(
    conn: m::mumble_connection_t,
    channel: m::mumble_channelid_t,
) {
    lock_plugin().plugin.on_channel_added(conn, channel);
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onChannelRemoved(
    conn: m::mumble_connection_t,
    channel: m::mumble_channelid_t,
) {
    lock_plugin().plugin.on_channel_removed(conn, channel);
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onChannelRenamed(
    conn: m::mumble_connection_t,
    channel: m::mumble_channelid_t,
) {
    lock_plugin().plugin.on_channel_renamed(conn, channel);
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_onKeyEvent(key_code: u32, pressed: bool) {
    lock_plugin().plugin.on_key_event(key_code, pressed);
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_hasUpdate() -> bool {
    let mut holder = lock_plugin();
    if let Some(updater) = &mut holder.updater {
        updater.has_update()
    } else {
        false
    }
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn mumble_getUpdateDownloadURL(
    buffer: *mut ::std::os::raw::c_char,
    buffer_size: u16,
    offset: u16,
) -> bool {
    if buffer_size == 0 { panic!("Cannot null-terminate empty recipient buffer"); }
    let offset = offset as usize;
    let buffer_size = buffer_size as usize;
    let mut holder = lock_plugin();
    let buffer = unsafe {
        std::slice::from_raw_parts_mut(buffer as *mut u8, buffer_size)
    };
    if let Some(updater) = &mut holder.updater {
        let url = updater.get_update_download_url();
        let url_bytes = url.as_bytes();
        if offset >= url_bytes.len() {
            buffer[0] = 0;
            return true
        }
        let offsetted = url_bytes.iter().skip(offset);
        let to_write = offsetted.take(buffer_size - 1).chain(&[0]);
        use ::collect_slice::CollectSlice;
        to_write.cloned().collect_slice(buffer);
        (url_bytes.len() - offset) >= (buffer_size - 1)
    } else {
        buffer[0] = 0;
        true
    }
}
