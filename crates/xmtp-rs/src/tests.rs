#[cfg(test)]
mod test {
  use super::super::*;
  use wasm_bindgen_test::*;

  wasm_bindgen_test_configure!(run_in_node_experimental);
  wasm_bindgen_test_configure!(run_in_browser);

  #[wasm_bindgen_test]
  async fn create_profile_test() {
    let profile = create_profile().await.unwrap();

  }
}