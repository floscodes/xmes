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
    let mut identities_toml = use_persistent("identities", || "".to_string());
    use_resource(move || async move {
        let identities = Identity::new(Env::Dev(Some("https://xmtp-dev.floscodes.net".to_string()))).await;
        match identities {
            Ok(identities) => identities_toml.set(format!("{}", identities.to_toml())),
            Err(e) => identities_toml.set(format!("Failed to create profile: {}", e)),
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
