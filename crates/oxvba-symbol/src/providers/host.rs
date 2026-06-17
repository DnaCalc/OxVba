//! The host object-model seam. The host's `Application`/`ThisWorkbook`/etc.
//! arrive as COM typelib metadata via host-injected references, so the host scope
//! is a `ComTypeLibProvider` over those host-injected blobs at host precedence.
//! There are no static Excel/Word stubs here — those are host COM metadata, not
//! the VBA language.

use oxvba_bundle::ProjectMemberKind;
use oxvba_com::TypeLibMetadataBlob;

use crate::binding::Binding;
use crate::provider::Provider;
use crate::providers::com::ComTypeLibProvider;
use crate::signature::VarTypeRef;

pub struct HostProvider {
    blobs: Vec<ComTypeLibProvider>,
}

impl HostProvider {
    pub fn new(blobs: Vec<TypeLibMetadataBlob>) -> Self {
        Self {
            blobs: blobs.into_iter().map(ComTypeLibProvider::new).collect(),
        }
    }
}

impl Provider for HostProvider {
    fn resolve(&self, name: &str) -> Option<Binding> {
        self.blobs.iter().find_map(|blob| blob.resolve(name))
    }

    fn resolve_member(
        &self,
        recv: &VarTypeRef,
        name: &str,
        want: Option<ProjectMemberKind>,
    ) -> Option<Binding> {
        self.blobs
            .iter()
            .find_map(|blob| blob.resolve_member(recv, name, want))
    }

    fn resolve_default_member(&self, recv: &VarTypeRef) -> Option<Binding> {
        self.blobs
            .iter()
            .find_map(|blob| blob.resolve_default_member(recv))
    }
}
