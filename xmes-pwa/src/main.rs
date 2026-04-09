// The dioxus prelude contains a ton of common items used in dioxus apps. It's a good idea to import wherever you
// need dioxus
use dioxus::prelude::*;
use dioxus_storage::use_persistent;
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
    let mut identity_toml = use_signal(|| "".to_string());
    use_resource(move || async move {
        let identity = Identity::new(Env::Dev(Some("https://xmtp-dev.floscodes.net".to_string()))).await;
        match identity {
            Ok(identity) => identity_toml.set(format!("TOML:\n{}", identity.to_toml())),
            Err(e) => identity_toml.set(format!("Failed to create profile: {}", e)),
        }     
    });

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        h1 {
            {identity_toml}
        }
    }
}
