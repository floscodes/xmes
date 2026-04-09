use dioxus::prelude::*;
use dioxus_sdk::storage::use_persistent;
use xmes_xmtp::{Env, Identity};

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
    use_resource(move || async move {
        if identities_toml().is_none() {
            let new_identity = Identity::new(Env::Dev(Some("https://xmtp-dev.floscodes.net".to_string()))).await.unwrap();
            identities_toml.set(Some(new_identity.to_toml()));
        }
    });

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        h1 {
            {identities_toml}
        }
    }
}
