use anyhow::Context;
use syn::{Item, ItemMod};

fn main() -> anyhow::Result<()> {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let spec_path = manifest.join("../openapi/chaos-tproxy.openapi.yaml");
    let out_path = manifest.join("src/generated.rs");

    println!("cargo:rerun-if-changed={}", spec_path.display());

    let spec_yaml = std::fs::read_to_string(&spec_path)
        .with_context(|| format!("read {}", spec_path.display()))?;
    let yaml_value: serde_yaml::Value = serde_yaml::from_str(&spec_yaml)?;
    let json = serde_json::to_string(&yaml_value)?;
    let spec: openapiv3::OpenAPI = serde_json::from_str(&json)?;

    let mut generator = progenitor::Generator::default();
    let tokens = generator
        .generate_tokens(&spec)
        .context("progenitor codegen failed")?;
    let file: syn::File = syn::parse2(tokens)?;

    // Extract only `pub mod types { ... }` and flatten its contents to the
    // top level — the empty HTTP client wrapper and progenitor_client imports
    // are dropped so chaos-tproxy-proxy doesn't need reqwest / progenitor_client.
    let types_mod: ItemMod = file
        .items
        .into_iter()
        .find_map(|item| match item {
            Item::Mod(m) if m.ident == "types" => Some(m),
            _ => None,
        })
        .context("no `pub mod types` found in progenitor output")?;
    let inner_items = types_mod
        .content
        .map(|(_, items)| items)
        .unwrap_or_default();

    let flattened = syn::File {
        shebang: None,
        attrs: Vec::new(),
        items: inner_items,
    };
    let content = prettyplease::unparse(&flattened);

    let header = "// @generated — do NOT edit by hand.\n\
                  // Re-run `cargo build -p chaos-tproxy-openapi` after editing\n\
                  // openapi/chaos-tproxy.openapi.yaml.\n\
                  //\n\
                  // Only the `types` module from progenitor's output is kept;\n\
                  // the (empty) HTTP client wrapper is intentionally dropped\n\
                  // so chaos-tproxy-proxy does not depend on reqwest /\n\
                  // progenitor_client.\n\n";

    std::fs::write(&out_path, format!("{header}{content}"))?;
    println!("cargo:warning=wrote {}", out_path.display());
    Ok(())
}
