Rust bindings for the Mumble Client Plugin API.

## Usage

Preliminary:

- Install Clang

- Download the Mumble source code and create a symlink named `mumble_sources`
  pointing to that directory (this crate extracts bindings from the source
  code's `plugins/` directory); alternatively set the env variable `MUMBLE_HOME`
  to that directory.

To use:

- Create a struct implementing `mumble_sys::traits::MumblePlugin`.

- Use [rust-ctor](https://crates.io/crates/ctor) to set an initializer
  which calls `mumble_sys::set_registration_callback(cb)`.

- Define `cb` to take a `mumble_sys::RegistrationToken` and return nothing.

- In the callback, instantiate your plugin and call `mumble_sys::register_plugin`
  with details of your plugin, and pass it the provided token.

- Your `MumblePlugin` can use the API given to it by `set_api` as long as it is set.
  It should be provided shortly after the call to `init` occurs.
  Feel free to multithread, just mutex the API given by `set_api`.
