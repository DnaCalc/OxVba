//! `oxvba-symbol` — the uniform VBA symbol model.
//!
//! One `Symbol` notion; a scope chain
//! `local → proc → module → sibling project modules → referenced projects →
//! VBA library → host → COM typelibs`; non-source scopes filled by `Provider`s;
//! a source-agnostic `resolve` (it never branches on source kind). It produces a
//! resolved `Binding`/`DispatchRoute` carrying exactly what the downstream binder
//! needs to emit a `oxvba_bundle::coreir::CoreCallee` — full native + COM interop
//! (early/late + events), cross-module/referenced projects, and every VBA
//! intrinsic including the quirky-syntax forms.

pub mod binding;
pub mod catalog;
pub mod manifest;
pub mod model;
pub mod predeclared;
pub mod provider;
pub mod providers;
pub mod scanner;
pub mod signature;
pub mod structural;
pub mod surface;

#[cfg(test)]
mod tests;

pub use binding::{Binding, DispatchRoute};
pub use model::{SymbolKind, SymbolModelError, SymbolTable};
pub use provider::{
    build_resolution_environment, CatalogTypeLibResolver, Provider, ResolutionContext,
    ResolutionEnvironment, TypeLibResolver,
};
