#![recursion_limit = "256"]

use std::rc::Rc;

use dioxus::prelude::*;
use dioxus_sdk::storage::use_persistent;
use xmes_xmtp_wasm::{Env, Identity};

mod components;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/styling/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut identities_toml: Signal<Option<String>> = use_persistent("identities", || None);
    let mut active_identity_address: Signal<Option<String>> = use_persistent("active_identity", || None);

    let mut identities: Signal<Vec<Rc<Identity>>> = use_signal(|| vec![]);
    let mut active_identity: Signal<Option<Rc<Identity>>> = use_signal(|| None);

    use_resource(move || async move {
        // Already initialized — a signal change triggered a re-run, but there's nothing to do.
        // Without this guard, setting identities_toml inside this resource would cause it to
        // re-run and call from_toml() while the client from new() still holds the OPFS file open,
        // leading to a conflict and an identity reset loop.
        if active_identity().is_some() {
            return;
        }

        let Some(toml) = identities_toml() else {
            let new_identity =
                Identity::new(Env::Dev(Some("https://xmtp-dev.floscodes.net".to_string())))
                    .await
                    .unwrap();
            active_identity_address.set(Some(new_identity.address()));
            identities_toml.set(Some(new_identity.to_toml()));
            active_identity.set(Some(Rc::new(new_identity)));
            return;
        };

        let Ok(loaded) = Identity::from_toml(toml).await else {
            // TOML is outdated or corrupt — reset and create a fresh identity
            identities_toml.set(None);
            active_identity_address.set(None);
            return;
        };
        let mut loaded: Vec<Rc<Identity>> = loaded.into_iter().map(Rc::new).collect();
        let active_idx = active_identity_address()
            .as_deref()
            .and_then(|addr| loaded.iter().position(|id| id.address() == addr))
            .unwrap_or(0);

        if !loaded.is_empty() {
            active_identity.set(Some(loaded.remove(active_idx)));
        }
        identities.set(loaded);
    });

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        components::conversations::Conversations { active_identity }
    }
}
