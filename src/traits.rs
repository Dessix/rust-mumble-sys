use crate::mumble::m;

#[allow(unused_variables)]
pub trait MumblePlugin: Send {
    fn shutdown(&self);

    //fn register_api_functions(api: m::MumbleAPI); // To be handled internally

    fn on_server_connected(&mut self, conn: m::ConnectionT) {}
    fn on_server_disconnected(&mut self, conn: m::ConnectionT) {}
    fn on_server_synchronized(&mut self, conn: m::ConnectionT) {}

    fn on_channel_entered(
        &mut self,
        conn: m::ConnectionT,
        user: m::UserIdT,
        previous: Option<m::ChannelIdT>,
        current: Option<m::ChannelIdT>,
    ) {
    }

    fn on_channel_exited(
        &mut self,
        conn: m::ConnectionT,
        user: m::UserIdT,
        channel: Option<m::ChannelIdT>,
    ) {
    }

    fn on_user_talking_state_changed(
        &mut self,
        conn: m::ConnectionT,
        user: m::UserIdT,
        talking_state: m::TalkingStateT,
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
        sample_rate: u32,
        is_speech: bool,
        user_id: Option<m::UserIdT>,
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
        conn: m::ConnectionT,
        sender: m::UserIdT,
        data_id: &str,
        decode_data: &dyn Fn() -> String,
    ) -> bool /* true if data consumed by this plugin */ {
        false
    }

    fn on_user_added(&mut self, conn: m::ConnectionT, user: m::UserIdT) {}

    fn on_user_removed(&mut self, conn: m::ConnectionT, user: m::UserIdT) {}

    fn on_channel_added(&mut self, conn: m::ConnectionT, channel: m::ChannelIdT) {}

    fn on_channel_removed(&mut self, conn: m::ConnectionT, channel: m::ChannelIdT) {}

    fn on_channel_renamed(&mut self, conn: m::ConnectionT, channel: m::ChannelIdT) {}

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

pub trait MumblePluginDescriptor: MumblePlugin {
    fn name() -> &'static str;
    fn author() -> &'static str;
    fn description() -> &'static str;
    fn version() -> m::Version {
        m::Version {
            major: 0,
            minor: 0,
            patch: 1,
        }
    }
    fn api_version() -> m::Version {
        unsafe { m::mumble_plugin_api_version.0 }
    }

    fn init(id: m::PluginId, api: m::MumbleAPI) -> Result<Self, m::ErrorT>
    where
        Self: Sized;
}
