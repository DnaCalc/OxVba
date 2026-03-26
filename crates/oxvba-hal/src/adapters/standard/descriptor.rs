use crate::model::{
    CapabilityDescriptor, CapabilityId, CapabilityMaturity, HalDescriptor, HalProfileId,
    HalRuntimeClass, HostPolicy, WasmRuntimeClass,
};

pub(crate) fn descriptor_for_profile(
    profile: HalProfileId,
    runtime_class: HalRuntimeClass,
    policy: &HostPolicy,
) -> HalDescriptor {
    HalDescriptor {
        profile,
        runtime_class: runtime_class.as_str(),
        contract_version: "hal-v2",
        adapter_version: "0.1.0",
        capabilities: capability_matrix(profile, runtime_class, policy.wasm_runtime_class),
    }
}

fn capability_matrix(
    profile: HalProfileId,
    runtime_class: HalRuntimeClass,
    wasm_runtime_class: WasmRuntimeClass,
) -> Vec<CapabilityDescriptor> {
    use CapabilityId as C;
    use CapabilityMaturity as M;
    let mut out = Vec::new();
    let mut push = |id: C, supported: bool, maturity: M, spec_anchor: &'static str| {
        out.push(CapabilityDescriptor {
            id,
            supported,
            maturity,
            spec_anchor,
        });
    };

    match profile {
        HalProfileId::Windows => {
            push(
                C::UiInteraction,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0337",
            );
            push(C::EventPump, true, M::Provisional, "MS-VBAL:DoEvents");
            push(
                C::FileSystemIo,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0286",
            );
            push(
                C::ProcessEnv,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0346",
            );
            push(
                C::ComActivationDispatch,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0325",
            );
            push(
                C::TimeLocale,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0252",
            );
            push(C::DynamicLinking, true, M::Provisional, "MS-VBAL:Declare");
            push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
            push(C::ProjectCatalog, false, M::Stable, "OxVba:HAL-PROJ");
            push(
                C::ProjectReferenceProvider,
                false,
                M::Stable,
                "OxVba:HAL-PROJ",
            );
            push(C::ProjectMutation, false, M::Stable, "OxVba:HAL-PROJ");
        }
        HalProfileId::Linux => {
            push(
                C::UiInteraction,
                true,
                M::Experimental,
                "CONF-discovered-ms-vbal-250520-f945507e-0337",
            );
            push(C::EventPump, true, M::Experimental, "MS-VBAL:DoEvents");
            push(
                C::FileSystemIo,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0286",
            );
            push(
                C::ProcessEnv,
                true,
                M::Experimental,
                "CONF-discovered-ms-vbal-250520-f945507e-0346",
            );
            push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
            push(
                C::TimeLocale,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0252",
            );
            push(C::DynamicLinking, true, M::Experimental, "MS-VBAL:Declare");
            push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
            push(C::ProjectCatalog, false, M::Stable, "OxVba:HAL-PROJ");
            push(
                C::ProjectReferenceProvider,
                false,
                M::Stable,
                "OxVba:HAL-PROJ",
            );
            push(C::ProjectMutation, false, M::Stable, "OxVba:HAL-PROJ");
        }
        HalProfileId::MacOs => {
            push(
                C::UiInteraction,
                true,
                M::Stub,
                "CONF-discovered-ms-vbal-250520-f945507e-0337",
            );
            push(C::EventPump, true, M::Stub, "MS-VBAL:DoEvents");
            push(
                C::FileSystemIo,
                true,
                M::Experimental,
                "CONF-discovered-ms-vbal-250520-f945507e-0286",
            );
            push(
                C::ProcessEnv,
                true,
                M::Experimental,
                "CONF-discovered-ms-vbal-250520-f945507e-0346",
            );
            push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
            push(
                C::TimeLocale,
                true,
                M::Experimental,
                "CONF-discovered-ms-vbal-250520-f945507e-0252",
            );
            push(C::DynamicLinking, true, M::Stub, "MS-VBAL:Declare");
            push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
            push(C::ProjectCatalog, false, M::Stable, "OxVba:HAL-PROJ");
            push(
                C::ProjectReferenceProvider,
                false,
                M::Stable,
                "OxVba:HAL-PROJ",
            );
            push(C::ProjectMutation, false, M::Stable, "OxVba:HAL-PROJ");
        }
        HalProfileId::Wasm => match runtime_class {
            HalRuntimeClass::WasmWasiLocal => {
                push(
                    C::UiInteraction,
                    true,
                    M::Experimental,
                    "CONF-discovered-ms-vbal-250520-f945507e-0337",
                );
                push(C::EventPump, true, M::Experimental, "MS-VBAL:DoEvents");
                push(C::FileSystemIo, false, M::Stable, "MS-VBAL:file-io");
                push(C::ProcessEnv, false, M::Stable, "MS-VBAL:Shell");
                push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
                push(
                    C::TimeLocale,
                    true,
                    M::Experimental,
                    "CONF-discovered-ms-vbal-250520-f945507e-0252",
                );
                push(C::DynamicLinking, false, M::Stable, "MS-VBAL:Declare");
                push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
                push(C::ProjectCatalog, false, M::Stable, "OxVba:HAL-PROJ");
                push(
                    C::ProjectReferenceProvider,
                    false,
                    M::Stable,
                    "OxVba:HAL-PROJ",
                );
                push(C::ProjectMutation, false, M::Stable, "OxVba:HAL-PROJ");
            }
            HalRuntimeClass::WasmBrowserSandbox => {
                push(
                    C::UiInteraction,
                    false,
                    M::Stable,
                    "MS-VBAL:MsgBox/InputBox",
                );
                push(C::EventPump, true, M::Experimental, "MS-VBAL:DoEvents");
                push(C::FileSystemIo, false, M::Stable, "MS-VBAL:file-io");
                push(C::ProcessEnv, false, M::Stable, "MS-VBAL:Shell");
                push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
                push(
                    C::TimeLocale,
                    true,
                    M::Experimental,
                    "CONF-discovered-ms-vbal-250520-f945507e-0252",
                );
                push(C::DynamicLinking, false, M::Stable, "MS-VBAL:Declare");
                push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
                push(C::ProjectCatalog, false, M::Stable, "OxVba:HAL-PROJ");
                push(
                    C::ProjectReferenceProvider,
                    false,
                    M::Stable,
                    "OxVba:HAL-PROJ",
                );
                push(C::ProjectMutation, false, M::Stable, "OxVba:HAL-PROJ");
            }
            _ => match wasm_runtime_class {
                WasmRuntimeClass::Wasi => {
                    push(
                        C::UiInteraction,
                        true,
                        M::Experimental,
                        "CONF-discovered-ms-vbal-250520-f945507e-0337",
                    );
                    push(C::EventPump, true, M::Experimental, "MS-VBAL:DoEvents");
                    push(C::FileSystemIo, false, M::Stable, "MS-VBAL:file-io");
                    push(C::ProcessEnv, false, M::Stable, "MS-VBAL:Shell");
                    push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
                    push(
                        C::TimeLocale,
                        true,
                        M::Experimental,
                        "CONF-discovered-ms-vbal-250520-f945507e-0252",
                    );
                    push(C::DynamicLinking, false, M::Stable, "MS-VBAL:Declare");
                    push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
                    push(C::ProjectCatalog, false, M::Stable, "OxVba:HAL-PROJ");
                    push(
                        C::ProjectReferenceProvider,
                        false,
                        M::Stable,
                        "OxVba:HAL-PROJ",
                    );
                    push(C::ProjectMutation, false, M::Stable, "OxVba:HAL-PROJ");
                }
                WasmRuntimeClass::BrowserSandbox => {
                    push(
                        C::UiInteraction,
                        false,
                        M::Stable,
                        "MS-VBAL:MsgBox/InputBox",
                    );
                    push(C::EventPump, true, M::Experimental, "MS-VBAL:DoEvents");
                    push(C::FileSystemIo, false, M::Stable, "MS-VBAL:file-io");
                    push(C::ProcessEnv, false, M::Stable, "MS-VBAL:Shell");
                    push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
                    push(
                        C::TimeLocale,
                        true,
                        M::Experimental,
                        "CONF-discovered-ms-vbal-250520-f945507e-0252",
                    );
                    push(C::DynamicLinking, false, M::Stable, "MS-VBAL:Declare");
                    push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
                    push(C::ProjectCatalog, false, M::Stable, "OxVba:HAL-PROJ");
                    push(
                        C::ProjectReferenceProvider,
                        false,
                        M::Stable,
                        "OxVba:HAL-PROJ",
                    );
                    push(C::ProjectMutation, false, M::Stable, "OxVba:HAL-PROJ");
                }
            },
        },
        HalProfileId::Null => {
            push(
                C::UiInteraction,
                false,
                M::Stable,
                "MS-VBAL:MsgBox/InputBox",
            );
            push(C::EventPump, false, M::Stable, "MS-VBAL:DoEvents");
            push(C::FileSystemIo, false, M::Stable, "MS-VBAL:file-io");
            push(C::ProcessEnv, false, M::Stable, "MS-VBAL:Shell/Dir/Environ");
            push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
            push(
                C::TimeLocale,
                true,
                M::Stable,
                "CONF-discovered-ms-vbal-250520-f945507e-0252",
            );
            push(C::DynamicLinking, false, M::Stable, "MS-VBAL:Declare");
            push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
            push(C::ProjectCatalog, false, M::Stable, "OxVba:HAL-PROJ");
            push(
                C::ProjectReferenceProvider,
                false,
                M::Stable,
                "OxVba:HAL-PROJ",
            );
            push(C::ProjectMutation, false, M::Stable, "OxVba:HAL-PROJ");
        }
    }
    out
}
