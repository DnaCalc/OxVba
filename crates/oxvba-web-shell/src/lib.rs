use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellAssetKind {
    Html,
    JavaScript,
    Css,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellAsset {
    pub path: &'static str,
    pub kind: ShellAssetKind,
    pub contents: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellManifest {
    pub app_name: &'static str,
    pub bridge_contract_version: &'static str,
    pub entry_asset_path: &'static str,
    pub assets: Vec<ShellAssetSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellAssetSummary {
    pub path: &'static str,
    pub kind: ShellAssetKind,
}

pub fn embedded_assets() -> Vec<ShellAsset> {
    vec![
        ShellAsset {
            path: "index.html",
            kind: ShellAssetKind::Html,
            contents: include_str!("../assets/index.html"),
        },
        ShellAsset {
            path: "app.js",
            kind: ShellAssetKind::JavaScript,
            contents: include_str!("../assets/app.js"),
        },
        ShellAsset {
            path: "styles.css",
            kind: ShellAssetKind::Css,
            contents: include_str!("../assets/styles.css"),
        },
    ]
}

pub fn shell_manifest() -> ShellManifest {
    let assets = embedded_assets()
        .into_iter()
        .map(|asset| ShellAssetSummary {
            path: asset.path,
            kind: asset.kind,
        })
        .collect();
    ShellManifest {
        app_name: "oxvba-web-shell",
        bridge_contract_version: "v1",
        entry_asset_path: "index.html",
        assets,
    }
}

#[cfg(test)]
mod tests {
    use super::{ShellAssetKind, embedded_assets, shell_manifest};

    #[test]
    fn embedded_assets_include_expected_frontend_files() {
        let assets = embedded_assets();
        assert_eq!(assets.len(), 3);
        assert_eq!(assets[0].path, "index.html");
        assert_eq!(assets[1].path, "app.js");
        assert_eq!(assets[2].path, "styles.css");
        assert!(assets.iter().all(|asset| !asset.contents.trim().is_empty()));
    }

    #[test]
    fn shell_manifest_matches_asset_inventory() {
        let manifest = shell_manifest();
        assert_eq!(manifest.app_name, "oxvba-web-shell");
        assert_eq!(manifest.bridge_contract_version, "v1");
        assert_eq!(manifest.entry_asset_path, "index.html");
        assert_eq!(manifest.assets.len(), 3);
        assert_eq!(manifest.assets[0].kind, ShellAssetKind::Html);
    }
}
