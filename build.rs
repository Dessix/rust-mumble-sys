#[macro_use]
extern crate const_format;
extern crate bindgen;

use heck;
use regex;
use std::env;
use std::fs;
use std::path::PathBuf;

const MUMBLE_NAME_ROOT: &'static str = "mumble";
const MUMBLE_WRAPPER_NAME: &'static str = concatcp!(MUMBLE_NAME_ROOT, "-wrapper.h");
const MUMBLE_BINDINGS_NAME: &'static str = concatcp!(MUMBLE_NAME_ROOT, ".rs");
const MUMBLE_WRAPPER_SRC: &'static str = "src/";
const MUMBLE_WRAPPER: &'static str = concatcp!(MUMBLE_WRAPPER_SRC, MUMBLE_WRAPPER_NAME);
const MUMBLE_BINDINGS: &'static str = MUMBLE_BINDINGS_NAME;

#[derive(Debug)]
struct CustomCallbacks {
    inner: bindgen::CargoCallbacks,
}

impl CustomCallbacks {
    pub fn new() -> Self {
        CustomCallbacks {
            inner: bindgen::CargoCallbacks,
        }
    }

    fn enum_name_handler(&self, original_variant_name: &str) -> Option<String> {
        // Mumble introduced enum prefixes into their APIs after this wrapper
        // was created. Rewrite them to match their initial expectations.
        // https://github.com/mumble-voip/mumble/commit/e9f0f711956b7739c320cc2012ab4b6037ffbda5
        let prefixed_mumble_enum_regex = regex::RegexBuilder::new(r"^MUMBLE_([A-Z]*?_.+)$")
            .build()
            .unwrap();

        match original_variant_name {
            // MUMBLE_TS_ is a special case---it was originally unprefixed.
            x if x.starts_with("MUMBLE_TS_") => {
                return Some(x["MUMBLE_TS_".len()..].into())
            }
            // MUMBLE_SK_ is a special case---it was originally partially prefixed.
            x if x.starts_with("MUMBLE_SK_") => {
                let suffix = &x["MUMBLE_".len()..];
                return Some(format!("M{}", suffix).into())
            }
            x if x.starts_with("MUMBLE_") && prefixed_mumble_enum_regex.is_match(x) => {
                return Some(prefixed_mumble_enum_regex.replace_all(x, "$1").into());
            }
            _ => {}
        }

        return Some(original_variant_name.into())
    }

    fn item_name_handler(&self, original_item_name: &str) -> Option<String> {
        if original_item_name == "root" {
            return Some("m".into());
        }
        let is_mumble_function_prefix = regex::RegexBuilder::new(r"^mumble_[a-z].+$")
            .build()
            .unwrap();
        match original_item_name {
            "Version" => return None,
            "MumbleVersion" => return Some("Version".into()),
            "MumbleStringWrapper" => return None,
            "mumble_plugin_id_t" => return Some("PluginId".into()),
            x if x.starts_with("MumbleAPI_") => return Some("MumbleAPI".into()),
            x if x.starts_with("Mumble_") && x.chars().filter(|x| *x == '_').count() == 1 => {
                return Some(x["Mumble_".len()..].into())
            }
            x if x.starts_with("mumble_")
                && !x.ends_with("_t")
                && is_mumble_function_prefix.is_match(&x) =>
            {
                return Some(x.into())
            }

            _ => {}
        }
        let strip_mumble_prefix = regex::RegexBuilder::new(r"^[Mm]umble_(.+)$")
            .build()
            .unwrap();

        let name: String = strip_mumble_prefix
            .replace(original_item_name, r"$1")
            .into();

        // methods should become snake case; types should become pascal case
        let name: String = if !name.ends_with("_t") {
            heck::SnekCase::to_snek_case(name.as_str()).into()
        } else {
            let name = name.replace("id_", "Id_");

            let unsnake = regex::RegexBuilder::new(r"(?:^|_)([a-z])").build().unwrap();

            let name = if name.starts_with(|c: char| c.is_alphabetic() && c.is_lowercase()) {
                unsnake
                    .replace_all(&name, |cap: &regex::Captures| {
                        cap.get(1).map(|m| m.as_str().to_uppercase()).unwrap()
                    })
                    .into()
            } else {
                name
            };

            name
        };
        Some(name)
    }
}

impl bindgen::callbacks::ParseCallbacks for CustomCallbacks {
    fn item_name(&self, original_item_name: &str) -> Option<String> {
        let new_name = self.item_name_handler(original_item_name);

        println!(
            "GEN NAME: {} = {}",
            original_item_name,
            match &new_name {
                Some(x) => x.as_str(),
                None => original_item_name,
            }
        );
        new_name
    }

    fn enum_variant_name(
            &self,
            _enum_name: Option<&str>,
            original_variant_name: &str,
            _variant_value: bindgen::callbacks::EnumVariantValue,
        ) -> Option<String> {
        let new_name = self.enum_name_handler(original_variant_name);

        println!(
            "GEN NAME: {} = {}",
            original_variant_name,
            match &new_name {
                Some(x) => x.as_str(),
                None => original_variant_name,
            }
        );
        new_name
    }

    fn include_file(&self, filename: &str) {
        self.inner.include_file(filename)
    }
}

fn main() {
    // println!("cargo:rerun-if-changed={}", MUMBLE_WRAPPER);
    env_logger::builder()
        .format(|buf, record| {
            use std::io::Write;
            writeln!(buf, "{}: {:#?}", record.level(), record.args())
        })
        .init();

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
                .expect("Must have a MUMBLE_HOME environment variable set if no mumble_sources symlink is present")
                .trim_end_matches("/")
                .to_string();
            if !PathBuf::from(&home_path).exists() {
                panic!(
                    "No mumble_sources directory in repo root, and MUMBLE_HOME env var referred to non-existent location"
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
            .clang_args(&["-x", "c++", "-std=c++20"])
            .clang_arg(format!("-I{}/plugins", mumble_home))
            // .layout_tests(false)
            .rust_target(bindgen::RustTarget::Nightly)
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
            .whitelist_type("MumbleAPI_v.*")
            // Mandatory functions
            .whitelist_function("mumble_init")
            .whitelist_function("mumble_shutdown")
            .whitelist_function("mumble_getName")
            .whitelist_function("mumble_getAPIVersion")
            .whitelist_function("mumble_registerAPIFunctions")
            .whitelist_function("mumble_releaseResource")
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
            // PluginComponents has references to std::string which apparently explode these days
            // We replace the ifdefs to get it to parse
            .header_contents(
                "PluginComponents_v_1_0_x.h",
                &std::fs::read_to_string(
                    PathBuf::from(&mumble_home)
                        .join("plugins")
                        .join("PluginComponents_v_1_0_x.h"),
                )
                .expect("PluginComponents file must exist")
                .replace("#ifdef __cplusplus", "#ifdef __never"),
            )
            .header(MUMBLE_WRAPPER)
            .parse_callbacks(Box::new(CustomCallbacks::new()))
            .generate()
            .expect("Unable to generate bindings");

        fs::write(&out_file, replace_fn_ptrs_nonnull(&bindings.to_string()))
            .expect("Couldn't write bindings!");
        println!("Wrote bindings to {:?}", &out_file);
    }
}
