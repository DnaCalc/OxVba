use std::{collections::BTreeMap, fs, path::PathBuf};

use oxvba_hal::model::{
    HalProfileId, HalRuntimeClass, HostPolicy, HostPolicyPreset, UiVirtualizationMode,
    UnsupportedFeatureMode, WasmRuntimeClass,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProfileId {
    WindowsGui,
    WindowsStdio,
    WindowsHeadless,
    LinuxStdio,
    WasmWasiLocal,
    WasmBrowserSandbox,
    NullFloor,
    MacOsHeadless,
}

impl RuntimeProfileId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WindowsGui => "windows-gui",
            Self::WindowsStdio => "windows-stdio",
            Self::WindowsHeadless => "windows-headless",
            Self::LinuxStdio => "linux-stdio",
            Self::WasmWasiLocal => "wasm-wasi-local",
            Self::WasmBrowserSandbox => "wasm-browser-sandbox",
            Self::NullFloor => "null-floor",
            Self::MacOsHeadless => "macos-headless",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "windows-gui" => Ok(Self::WindowsGui),
            "windows-stdio" => Ok(Self::WindowsStdio),
            "windows-headless" => Ok(Self::WindowsHeadless),
            "linux-stdio" => Ok(Self::LinuxStdio),
            "wasm-wasi-local" => Ok(Self::WasmWasiLocal),
            "wasm-browser-sandbox" => Ok(Self::WasmBrowserSandbox),
            "null-floor" => Ok(Self::NullFloor),
            "macos-headless" => Ok(Self::MacOsHeadless),
            other => Err(format!("invalid runtime profile: {other}")),
        }
    }

    pub const fn hal_profile(self) -> HalProfileId {
        match self {
            Self::WindowsGui | Self::WindowsStdio | Self::WindowsHeadless => HalProfileId::Windows,
            Self::LinuxStdio => HalProfileId::Linux,
            Self::WasmWasiLocal | Self::WasmBrowserSandbox => HalProfileId::Wasm,
            Self::NullFloor => HalProfileId::Null,
            Self::MacOsHeadless => HalProfileId::MacOs,
        }
    }

    pub const fn runtime_class(self) -> HalRuntimeClass {
        match self {
            Self::WindowsGui => HalRuntimeClass::WindowsGui,
            Self::WindowsStdio => HalRuntimeClass::WindowsStdio,
            Self::WindowsHeadless => HalRuntimeClass::WindowsHeadless,
            Self::LinuxStdio => HalRuntimeClass::LinuxStdio,
            Self::WasmWasiLocal => HalRuntimeClass::WasmWasiLocal,
            Self::WasmBrowserSandbox => HalRuntimeClass::WasmBrowserSandbox,
            Self::NullFloor => HalRuntimeClass::NullFloor,
            Self::MacOsHeadless => HalRuntimeClass::MacOsHeadless,
        }
    }

    pub const fn default_for_hal_profile(profile: HalProfileId) -> Self {
        match profile {
            HalProfileId::Windows => Self::WindowsStdio,
            HalProfileId::Linux => Self::LinuxStdio,
            HalProfileId::MacOs => Self::MacOsHeadless,
            HalProfileId::Wasm => Self::WasmWasiLocal,
            HalProfileId::Null => Self::NullFloor,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicyOverrides {
    pub runtime_class: Option<HalRuntimeClass>,
    pub allow_interaction: Option<bool>,
    pub allow_process_spawn: Option<bool>,
    pub allow_filesystem_mutation: Option<bool>,
    pub allow_dynamic_link: Option<bool>,
    pub allow_com_activation: Option<bool>,
    pub deterministic_mode: Option<bool>,
    pub ui_virtualization: Option<UiVirtualizationMode>,
    pub unsupported_feature_mode: Option<UnsupportedFeatureMode>,
    pub wasm_runtime_class: Option<WasmRuntimeClass>,
}

impl PolicyOverrides {
    pub fn apply_to(&self, policy: &mut HostPolicy) {
        if let Some(value) = self.runtime_class {
            policy.runtime_class = Some(value);
        }
        if let Some(value) = self.allow_interaction {
            policy.allow_interaction = value;
        }
        if let Some(value) = self.allow_process_spawn {
            policy.allow_process_spawn = value;
        }
        if let Some(value) = self.allow_filesystem_mutation {
            policy.allow_filesystem_mutation = value;
        }
        if let Some(value) = self.allow_dynamic_link {
            policy.allow_dynamic_link = value;
        }
        if let Some(value) = self.allow_com_activation {
            policy.allow_com_activation = value;
        }
        if let Some(value) = self.deterministic_mode {
            policy.deterministic_mode = value;
        }
        if let Some(value) = self.ui_virtualization {
            policy.ui_virtualization = value;
        }
        if let Some(value) = self.unsupported_feature_mode {
            policy.unsupported_feature_mode = value;
        }
        if let Some(value) = self.wasm_runtime_class {
            policy.wasm_runtime_class = value;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunnerBootstrapOptions {
    pub config_path: Option<PathBuf>,
    pub profile: Option<String>,
    pub policy_preset: Option<String>,
    pub overrides: PolicyOverrides,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRunnerBootstrap {
    pub runtime_profile: RuntimeProfileId,
    pub policy_preset: HostPolicyPreset,
    pub policy: HostPolicy,
    pub explicit_overrides: Vec<String>,
}

impl ResolvedRunnerBootstrap {
    pub fn fingerprint(&self) -> String {
        let mut overrides = self.explicit_overrides.clone();
        overrides.sort();
        let overrides_text = if overrides.is_empty() {
            "none".to_string()
        } else {
            overrides.join(",")
        };
        format!(
            "profile={};hal_profile={:?};runtime_class={};policy_preset={};deterministic={};overrides={}",
            self.runtime_profile.as_str(),
            self.runtime_profile.hal_profile(),
            self.runtime_profile.runtime_class().as_str(),
            self.policy_preset.as_str(),
            self.policy.deterministic_mode,
            overrides_text
        )
    }
}

pub fn resolve_runner_bootstrap(
    options: &RunnerBootstrapOptions,
    env_get: impl Fn(&str) -> Option<String>,
) -> Result<ResolvedRunnerBootstrap, String> {
    let mut merged = BTreeMap::<String, String>::new();

    if let Some(path) = &options.config_path {
        for (key, value) in parse_config_file(path)? {
            merged.insert(key, value);
        }
    }
    for (key, value) in env_key_values(env_get) {
        merged.insert(key, value);
    }
    if let Some(profile) = &options.profile {
        merged.insert("profile".to_string(), profile.clone());
    }
    if let Some(preset) = &options.policy_preset {
        merged.insert("policy_preset".to_string(), preset.clone());
    }
    if let Some(value) = options.overrides.runtime_class {
        merged.insert("runtime_class".to_string(), value.as_str().to_string());
    }
    if let Some(value) = options.overrides.allow_interaction {
        merged.insert("allow_interaction".to_string(), value.to_string());
    }
    if let Some(value) = options.overrides.allow_process_spawn {
        merged.insert("allow_process_spawn".to_string(), value.to_string());
    }
    if let Some(value) = options.overrides.allow_filesystem_mutation {
        merged.insert("allow_filesystem_mutation".to_string(), value.to_string());
    }
    if let Some(value) = options.overrides.allow_dynamic_link {
        merged.insert("allow_dynamic_link".to_string(), value.to_string());
    }
    if let Some(value) = options.overrides.allow_com_activation {
        merged.insert("allow_com_activation".to_string(), value.to_string());
    }
    if let Some(value) = options.overrides.deterministic_mode {
        merged.insert("deterministic_mode".to_string(), value.to_string());
    }
    if let Some(value) = options.overrides.ui_virtualization {
        merged.insert(
            "ui_virtualization".to_string(),
            match value {
                UiVirtualizationMode::Disabled => "disabled",
                UiVirtualizationMode::ScriptedResponses => "scripted-responses",
                UiVirtualizationMode::FailOnPrompt => "fail-on-prompt",
                UiVirtualizationMode::HostCallback => "host-callback",
            }
            .to_string(),
        );
    }
    if let Some(value) = options.overrides.unsupported_feature_mode {
        merged.insert(
            "unsupported_feature_mode".to_string(),
            match value {
                UnsupportedFeatureMode::CompileTime => "compile-time",
                UnsupportedFeatureMode::Runtime => "runtime",
            }
            .to_string(),
        );
    }
    if let Some(value) = options.overrides.wasm_runtime_class {
        merged.insert(
            "wasm_runtime_class".to_string(),
            match value {
                WasmRuntimeClass::Wasi => "wasi",
                WasmRuntimeClass::BrowserSandbox => "browser-sandbox",
            }
            .to_string(),
        );
    }

    let runtime_profile = merged
        .get("profile")
        .map_or(Ok(RuntimeProfileId::default_for_hal_profile(if cfg!(target_os = "windows") {
            HalProfileId::Windows
        } else if cfg!(target_os = "linux") {
            HalProfileId::Linux
        } else if cfg!(target_os = "macos") {
            HalProfileId::MacOs
        } else {
            HalProfileId::Null
        })), |raw| RuntimeProfileId::parse(raw))?;
    let policy_preset = merged
        .get("policy_preset")
        .map_or(Ok(HostPolicyPreset::DeterministicRuntime), |raw| {
            parse_policy_preset(raw)
        })?;

    let mut policy = HostPolicy::for_preset(policy_preset);
    policy.runtime_class = Some(runtime_profile.runtime_class());
    match runtime_profile {
        RuntimeProfileId::WindowsGui => {
            policy.allow_interaction = true;
            if policy.ui_virtualization == UiVirtualizationMode::ScriptedResponses {
                policy.ui_virtualization = UiVirtualizationMode::Disabled;
            }
        }
        RuntimeProfileId::WindowsStdio | RuntimeProfileId::LinuxStdio => {
            policy.allow_interaction = true;
        }
        RuntimeProfileId::WasmWasiLocal => {
            policy.wasm_runtime_class = WasmRuntimeClass::Wasi;
        }
        RuntimeProfileId::WasmBrowserSandbox => {
            policy.wasm_runtime_class = WasmRuntimeClass::BrowserSandbox;
            policy.allow_interaction = false;
        }
        RuntimeProfileId::NullFloor
        | RuntimeProfileId::WindowsHeadless
        | RuntimeProfileId::MacOsHeadless => {}
    }

    let mut explicit_overrides = Vec::new();
    for (key, value) in &merged {
        match key.as_str() {
            "runtime_class" => {
                policy.runtime_class = Some(parse_runtime_class(value)?);
                explicit_overrides.push(key.clone());
            }
            "allow_interaction" => {
                policy.allow_interaction = parse_bool(value, key)?;
                explicit_overrides.push(key.clone());
            }
            "allow_process_spawn" => {
                policy.allow_process_spawn = parse_bool(value, key)?;
                explicit_overrides.push(key.clone());
            }
            "allow_filesystem_mutation" => {
                policy.allow_filesystem_mutation = parse_bool(value, key)?;
                explicit_overrides.push(key.clone());
            }
            "allow_dynamic_link" => {
                policy.allow_dynamic_link = parse_bool(value, key)?;
                explicit_overrides.push(key.clone());
            }
            "allow_com_activation" => {
                policy.allow_com_activation = parse_bool(value, key)?;
                explicit_overrides.push(key.clone());
            }
            "deterministic_mode" => {
                policy.deterministic_mode = parse_bool(value, key)?;
                explicit_overrides.push(key.clone());
            }
            "ui_virtualization" => {
                policy.ui_virtualization = parse_ui_virtualization(value)?;
                explicit_overrides.push(key.clone());
            }
            "unsupported_feature_mode" => {
                policy.unsupported_feature_mode = parse_unsupported_feature_mode(value)?;
                explicit_overrides.push(key.clone());
            }
            "wasm_runtime_class" => {
                policy.wasm_runtime_class = parse_wasm_runtime_class(value)?;
                explicit_overrides.push(key.clone());
            }
            "profile" | "policy_preset" => {}
            _ => return Err(format!("unknown host-runner key: {key}")),
        }
    }

    Ok(ResolvedRunnerBootstrap {
        runtime_profile,
        policy_preset,
        policy,
        explicit_overrides,
    })
}

fn env_key_values(env_get: impl Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    const ENV_KEYS: &[(&str, &str)] = &[
        ("OXVBA_PROFILE", "profile"),
        ("OXVBA_POLICY_PRESET", "policy_preset"),
        ("OXVBA_RUNTIME_CLASS", "runtime_class"),
        ("OXVBA_ALLOW_INTERACTION", "allow_interaction"),
        ("OXVBA_ALLOW_PROCESS_SPAWN", "allow_process_spawn"),
        (
            "OXVBA_ALLOW_FILESYSTEM_MUTATION",
            "allow_filesystem_mutation",
        ),
        ("OXVBA_ALLOW_DYNAMIC_LINK", "allow_dynamic_link"),
        ("OXVBA_ALLOW_COM_ACTIVATION", "allow_com_activation"),
        ("OXVBA_DETERMINISTIC_MODE", "deterministic_mode"),
        ("OXVBA_UI_VIRTUALIZATION", "ui_virtualization"),
        ("OXVBA_UNSUPPORTED_FEATURE_MODE", "unsupported_feature_mode"),
        ("OXVBA_WASM_RUNTIME_CLASS", "wasm_runtime_class"),
    ];
    let mut out = Vec::new();
    for (env_key, key) in ENV_KEYS {
        if let Some(value) = env_get(env_key) {
            out.push(((*key).to_string(), value));
        }
    }
    out
}

fn parse_config_file(path: &PathBuf) -> Result<BTreeMap<String, String>, String> {
    let text = fs::read_to_string(path).map_err(|err| {
        format!(
            "failed to read host-runner config {}: {err}",
            path.display()
        )
    })?;
    parse_config_text(&text)
}

fn parse_config_text(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut in_host_section = false;
    let mut out = BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_host_section = line.eq_ignore_ascii_case("[host]");
            continue;
        }
        if !in_host_section {
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            return Err(format!("invalid host-runner config line: {line}"));
        };
        let key = key.trim().to_ascii_lowercase();
        let value = raw_value.trim().trim_matches('"').to_string();
        out.insert(key, value);
    }
    Ok(out)
}

fn parse_policy_preset(raw: &str) -> Result<HostPolicyPreset, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "strict-ci" => Ok(HostPolicyPreset::StrictCi),
        "deterministic-runtime" => Ok(HostPolicyPreset::DeterministicRuntime),
        "deterministic-compile-time" => Ok(HostPolicyPreset::DeterministicCompileTime),
        "interactive-dev" => Ok(HostPolicyPreset::InteractiveDev),
        other => Err(format!("invalid policy preset: {other}")),
    }
}

fn parse_runtime_class(raw: &str) -> Result<HalRuntimeClass, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "host-native" => Ok(HalRuntimeClass::HostNative),
        "windows-gui" => Ok(HalRuntimeClass::WindowsGui),
        "windows-stdio" => Ok(HalRuntimeClass::WindowsStdio),
        "windows-headless" => Ok(HalRuntimeClass::WindowsHeadless),
        "linux-stdio" => Ok(HalRuntimeClass::LinuxStdio),
        "linux-headless" => Ok(HalRuntimeClass::LinuxHeadless),
        "macos-gui" => Ok(HalRuntimeClass::MacOsGui),
        "macos-headless" => Ok(HalRuntimeClass::MacOsHeadless),
        "wasi-local" => Ok(HalRuntimeClass::WasmWasiLocal),
        "browser-sandbox" => Ok(HalRuntimeClass::WasmBrowserSandbox),
        "null-floor" => Ok(HalRuntimeClass::NullFloor),
        other => Err(format!("invalid runtime class: {other}")),
    }
}

fn parse_bool(raw: &str, key: &str) -> Result<bool, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        other => Err(format!("invalid boolean value for {key}: {other}")),
    }
}

fn parse_ui_virtualization(raw: &str) -> Result<UiVirtualizationMode, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "disabled" => Ok(UiVirtualizationMode::Disabled),
        "scripted-responses" => Ok(UiVirtualizationMode::ScriptedResponses),
        "fail-on-prompt" => Ok(UiVirtualizationMode::FailOnPrompt),
        other => Err(format!("invalid ui_virtualization: {other}")),
    }
}

fn parse_unsupported_feature_mode(raw: &str) -> Result<UnsupportedFeatureMode, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "compile-time" => Ok(UnsupportedFeatureMode::CompileTime),
        "runtime" => Ok(UnsupportedFeatureMode::Runtime),
        other => Err(format!("invalid unsupported_feature_mode: {other}")),
    }
}

fn parse_wasm_runtime_class(raw: &str) -> Result<WasmRuntimeClass, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "wasi" | "wasi-local" => Ok(WasmRuntimeClass::Wasi),
        "browser-sandbox" => Ok(WasmRuntimeClass::BrowserSandbox),
        other => Err(format!("invalid wasm_runtime_class: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use oxvba_hal::model::{HostPolicyPreset, UiVirtualizationMode, UnsupportedFeatureMode};

    use super::{
        PolicyOverrides, RunnerBootstrapOptions, RuntimeProfileId, parse_config_text,
        resolve_runner_bootstrap,
    };

    #[test]
    fn bootstrap_precedence_cli_over_env_over_config() {
        let config = r#"
[host]
profile = "windows-headless"
policy_preset = "strict-ci"
allow_interaction = false
"#;
        let config_path = PathBuf::from("temp/runner_bootstrap_precedence.toml");
        std::fs::create_dir_all("temp").expect("temp dir");
        std::fs::write(&config_path, config).expect("write config");

        let options = RunnerBootstrapOptions {
            config_path: Some(config_path.clone()),
            profile: Some("linux-stdio".to_string()),
            policy_preset: Some("interactive-dev".to_string()),
            overrides: PolicyOverrides {
                allow_interaction: Some(false),
                ..PolicyOverrides::default()
            },
        };
        let resolved = resolve_runner_bootstrap(&options, |key| match key {
            "OXVBA_POLICY_PRESET" => Some("deterministic-runtime".to_string()),
            _ => None,
        })
        .expect("bootstrap resolve");
        assert_eq!(resolved.runtime_profile, RuntimeProfileId::LinuxStdio);
        assert_eq!(resolved.policy_preset, HostPolicyPreset::InteractiveDev);
        assert!(!resolved.policy.allow_interaction);

        let _ = std::fs::remove_file(config_path);
    }

    #[test]
    fn bootstrap_fingerprint_is_deterministic() {
        let options = RunnerBootstrapOptions {
            profile: Some("windows-headless".to_string()),
            policy_preset: Some("deterministic-runtime".to_string()),
            overrides: PolicyOverrides {
                unsupported_feature_mode: Some(UnsupportedFeatureMode::CompileTime),
                ui_virtualization: Some(UiVirtualizationMode::ScriptedResponses),
                ..PolicyOverrides::default()
            },
            ..RunnerBootstrapOptions::default()
        };
        let resolved = resolve_runner_bootstrap(&options, |_| None).expect("resolve");
        let first = resolved.fingerprint();
        let second = resolved.fingerprint();
        assert_eq!(first, second);
        assert!(first.contains("profile=windows-headless"));
        assert!(first.contains("policy_preset=deterministic-runtime"));
    }

    #[test]
    fn parse_config_text_reads_host_section_only() {
        let text = r#"
[other]
profile = "null-floor"

[host]
profile = "windows-gui"
allow_interaction = true
"#;
        let parsed = parse_config_text(text).expect("parse config");
        assert_eq!(
            parsed.get("profile").map(String::as_str),
            Some("windows-gui")
        );
        assert_eq!(
            parsed.get("allow_interaction").map(String::as_str),
            Some("true")
        );
        assert!(!parsed.contains_key("policy_preset"));
    }
}
