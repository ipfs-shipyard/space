use messages::ApplicationAPI;
// The file has been placed there by the build script.
include!(concat!(env!("OUT_DIR"), "/built.rs"));

pub fn get(remote_label: Option<String>) -> ApplicationAPI {
    ApplicationAPI::Version {
        version: env!("CARGO_PKG_VERSION").to_string(),
        rust: env!("CARGO_PKG_RUST_VERSION").to_string(),
        target: TARGET.to_owned(),
        profile: PROFILE.to_owned(),
        features: FEATURES.iter().map(|s| s.to_string()).collect(),
        remote_label,
    }
}
