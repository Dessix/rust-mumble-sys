use crate::mumble::root as m;

#[allow(unused_variables)]
pub trait MumblePlugin {
    // Mandatory functions
    fn init(&mut self) -> m::mumble_error_t;
    fn shutdown(&mut self);

    fn set_api(&mut self, api: crate::MumbleAPI);
    //fn register_api_functions(api: m::MumbleAPI); // To be handled internally

    fn on_server_connected(&mut self, conn: m::mumble_connection_t) {}
    fn on_server_disconnected(&mut self, conn: m::mumble_connection_t) {}
    fn on_server_synchronized(&mut self, conn: m::mumble_connection_t) {}

    fn on_channel_entered(
        &mut self,
        conn: m::mumble_connection_t,
        user: m::mumble_userid_t,
        previous: Option<m::mumble_channelid_t>,
        current: Option<m::mumble_channelid_t>,
    ) {
    }

    fn on_channel_exited(
        &mut self,
        conn: m::mumble_connection_t,
        user: m::mumble_userid_t,
        channel: Option<m::mumble_channelid_t>,
    ) {
    }

    fn on_user_talking_state_changed(
        &mut self,
        conn: m::mumble_connection_t,
        user: m::mumble_userid_t,
        talking_state: m::talking_state_t,
    ) {
    }

    fn on_audio_input(
        &mut self,
        pulse_code_modulation: &mut [i16], // Length is sample_count * channel_count
        sample_count: u32,
        channel_count: u16,
        is_speech: bool,
    ) -> bool /* true if mutated */ {
        false
    }

    fn on_audio_source_fetched(
        &mut self,
        pcm: &mut [f32],
        sample_count: u32,
        channel_count: u16,
        is_speech: bool,
        user_id: Option<m::mumble_userid_t>,
    ) -> bool {
        false
    }

    fn on_audio_output_about_to_play(
        &mut self,
        pcm: &mut [f32],
        sample_count: u32,
        channel_count: u16,
    ) -> bool {
        false
    }

    fn on_receive_data(
        &mut self,
        conn: m::mumble_connection_t,
        sender: m::mumble_userid_t,
        data_id: &str,
        decode_data: &dyn Fn() -> String,
    ) -> bool /* true if data consumed by this plugin */ {
        false
    }

    fn on_user_added(&mut self, conn: m::mumble_connection_t, user: m::mumble_userid_t) {}

    fn on_user_removed(&mut self, conn: m::mumble_connection_t, user: m::mumble_userid_t) {}

    fn on_channel_added(&mut self, conn: m::mumble_connection_t, channel: m::mumble_channelid_t) {}

    fn on_channel_removed(&mut self, conn: m::mumble_connection_t, channel: m::mumble_channelid_t) {
    }

    fn on_channel_renamed(&mut self, conn: m::mumble_connection_t, channel: m::mumble_channelid_t) {
    }

    fn on_key_event(&mut self, key_code: u32, pressed: bool) {}
}

pub trait MumblePluginUpdater {
    fn has_update(&mut self) -> bool {
        false
    }

    fn get_update_download_url(&mut self) -> String {
        String::new()
    }
}

pub trait CheckableId
where
    Self: Sized,
{
    fn check(self) -> Option<Self>;
}

pub trait ErrAsResult
where
    Self: Sized,
{
    type ErrType;
    fn resultify(self) -> Result<Self, Self::ErrType>;
}
