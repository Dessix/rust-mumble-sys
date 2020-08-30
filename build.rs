#![feature(nll)]

#[macro_use]
extern crate const_concat;
extern crate bindgen;

use regex;
use std::env;
use std::fs;
use std::path::PathBuf;

const MUMBLE_NAME_ROOT: &'static str = "mumble";
const MUMBLE_WRAPPER_NAME: &'static str = const_concat!(MUMBLE_NAME_ROOT, "-wrapper.h");
const MUMBLE_BINDINGS_NAME: &'static str = const_concat!(MUMBLE_NAME_ROOT, ".rs");
const MUMBLE_WRAPPER_SRC: &'static str = "src/";
const MUMBLE_WRAPPER: &'static str = const_concat!(MUMBLE_WRAPPER_SRC, MUMBLE_WRAPPER_NAME);
const MUMBLE_BINDINGS: &'static str = MUMBLE_BINDINGS_NAME;

fn main() {
    println!("cargo:rerun-if-changed={}", MUMBLE_WRAPPER);

    // let out_file = if cfg!(feature = "idebuild") {
    let out_dir = env::current_dir().unwrap();
    let out_file = out_dir.join(MUMBLE_WRAPPER_SRC).join(MUMBLE_BINDINGS);
    // } else {
    //     let out_dir = std::path::PathBuf::from(env::var("OUT_DIR").unwrap());
    //     let out_file = out_dir.join(MUMBLE_BINDINGS);
    //     out_file
    // };

    let should_build = !out_file.exists() || true; //TODO: Remove || true
    if should_build {
        let mumble_sources_symlink = env::current_dir().unwrap().join("mumble_sources");
        let mumble_home = if mumble_sources_symlink.exists() {
            mumble_sources_symlink
                .canonicalize()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        } else {
            let home_path = env::var("MUMBLE_HOME")
                .unwrap()
                .trim_end_matches("/")
                .to_string();
            if !PathBuf::from(&home_path).exists() {
                panic!(
                    "No mumble_sources directory in repo root, and no MUMBLE_HOME env var defined"
                )
            }
            home_path
        };

        // TODO: This should be done with procedural macros instead
        let regex_replace_with_nonnull = regex::RegexBuilder::new(
            r"\bpub ([^\s:]+): (?:(?:::std::)?option::)?Option<\s*(.+?,)\s*>,\n",
        )
        .dot_matches_new_line(true)
        .build()
        .unwrap();
        let replace_fn_ptrs_nonnull = |s: &str| -> String {
            regex_replace_with_nonnull
                .replace_all(s, "pub ${1}: $2\n")
                .to_string()
        };

        let bindings = bindgen::Builder::default()
            .clang_args(&["-x", "c++", "-std=c++17"])
            .clang_arg(format!("-I{}/plugins", mumble_home))
            // .layout_tests(false)
            .enable_cxx_namespaces()
            .default_enum_style(bindgen::EnumVariation::Rust {
                non_exhaustive: false,
            })
            .default_alias_style(bindgen::AliasVariation::NewTypeDeref)
            .type_alias("mumble_error_t")
            .derive_eq(true)
            .size_t_is_usize(true)
            // <allow-list>
            .whitelist_var("MUMBLE_PLUGIN_API_VERSION")
            .whitelist_type("MumbleAPI")
            // Mandatory functions
            .whitelist_function("mumble_init")
            .whitelist_function("mumble_shutdown")
            .whitelist_function("mumble_getName")
            .whitelist_function("mumble_getAPIVersion")
            .whitelist_function("mumble_registerAPIFunctions")
            // General functions
            .whitelist_function("mumble_setMumbleInfo")
            .whitelist_function("mumble_getVersion")
            .whitelist_function("mumble_getAuthor")
            .whitelist_function("mumble_getDescription")
            .whitelist_function("mumble_registerPluginID")
            .whitelist_function("mumble_getFeatures")
            .whitelist_function("mumble_deactivateFeatures")
            // Positional audio
            .whitelist_function("mumble_initPositionalData")
            .whitelist_function("mumble_fetchPositionalData")
            .whitelist_function("mumble_shutdownPositionalData")
            // EventHandlers / Callback Functions
            .whitelist_function("mumble_onServerConnected")
            .whitelist_function("mumble_onServerDisconnected")
            .whitelist_function("mumble_onServerSynchronized")
            .whitelist_function("mumble_onChannelEntered")
            .whitelist_function("mumble_onChannelExited")
            .whitelist_function("mumble_onUserTalkingStateChanged")
            .whitelist_function("mumble_onAudioInput")
            .whitelist_function("mumble_onAudioSourceFetched")
            .whitelist_function("mumble_onAudioOutputAboutToPlay")
            .whitelist_function("mumble_onReceiveData")
            .whitelist_function("mumble_onUserAdded")
            .whitelist_function("mumble_onUserRemoved")
            .whitelist_function("mumble_onChannelAdded")
            .whitelist_function("mumble_onChannelRemoved")
            .whitelist_function("mumble_onChannelRenamed")
            .whitelist_function("mumble_onKeyEvent")
            // Plugin updates
            .whitelist_function("mumble_hasUpdate")
            .whitelist_function("mumble_getUpdateDownloadURL")
            // </allow-list>
            .detect_include_paths(true)
            .header(MUMBLE_WRAPPER)
            .parse_callbacks(Box::new(bindgen::CargoCallbacks))
            .generate()
            .expect("Unable to generate bindings");

        fs::write(out_file, replace_fn_ptrs_nonnull(&bindings.to_string()))
            .expect("Couldn't write bindings!");
    }
}
