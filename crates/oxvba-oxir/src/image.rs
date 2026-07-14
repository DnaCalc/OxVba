//! `OxImage` — the linkable set of `OxProgram`s for one VBA *project closure*, and the vm3
//! runtime artifact (`.oxi`).
//!
//! A whole VBA project is exactly one [`OxProgram`]; cross-project references add more, with the
//! entry (the COM-server / startup project) LAST — the same ordering [`crate`]'s consumer
//! `oxvba_vm3::Vm3::link` uses. This is the vm3 runtime artifact: a build emits it (by
//! elaborating each project's `CoreProgram`) for the in-process COM server to load and link.
//! `OxProgram` is fully serde-serializable, so the image round-trips through JSON.

use crate::program::OxProgram;
use serde::{Deserialize, Serialize};

/// On-disk format tag for an `OxImage` artifact (rejects a stale/foreign blob via its header).
pub const OX_IMAGE_FORMAT: &str = "oxvba.ox-image";
/// On-disk schema version.
pub const OX_IMAGE_VERSION: u32 = 2;

/// A linkable image: the elaborated programs of a project closure plus the entry index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxImage {
    pub format: String,
    pub version: u32,
    /// Index into `programs` of the entry project (whose `Main`/exported classes the host
    /// drives). The cross-project entry is LAST, matching `Vm3::link`.
    pub entry: usize,
    pub programs: Vec<OxProgram>,
}

/// Errors building or loading an [`OxImage`].
#[derive(Debug)]
pub enum OxImageError {
    Serialize(serde_json::Error),
    Deserialize(serde_json::Error),
    UnknownFormat { format: String },
    UnsupportedVersion { version: u32 },
    Empty,
    EntryOutOfRange { entry: usize, count: usize },
    /// A contained program failed structural verification (e.g. an out-of-range
    /// entry/func/block reference in a corrupt or foreign `.oxi`).
    VerificationFailed {
        program: usize,
        errors: Vec<crate::verify::VerifyError>,
    },
}

impl std::fmt::Display for OxImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OxImageError::Serialize(e) => write!(f, "OxImage serialize: {e}"),
            OxImageError::Deserialize(e) => write!(f, "OxImage deserialize: {e}"),
            OxImageError::UnknownFormat { format } => {
                write!(f, "not an OxImage artifact: format `{format}`")
            }
            OxImageError::UnsupportedVersion { version } => {
                write!(f, "unsupported OxImage version {version}")
            }
            OxImageError::Empty => write!(f, "OxImage has no programs"),
            OxImageError::EntryOutOfRange { entry, count } => {
                write!(f, "OxImage entry {entry} out of range ({count} programs)")
            }
            OxImageError::VerificationFailed { program, errors } => {
                write!(f, "OxImage program {program} failed verification:")?;
                for e in errors {
                    write!(f, " {e};")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for OxImageError {}

impl OxImage {
    /// Build an image from the elaborated programs of a closure, with the entry as the LAST
    /// program (the closure ordering the build and `Vm3::link` use). An empty `programs` yields
    /// `entry = 0`, which [`validate`](Self::validate) then rejects.
    pub fn new(programs: Vec<OxProgram>) -> Self {
        let entry = programs.len().saturating_sub(1);
        OxImage {
            format: OX_IMAGE_FORMAT.to_string(),
            version: OX_IMAGE_VERSION,
            entry,
            programs,
        }
    }

    /// Serialize to the `.oxi` byte form (pretty JSON).
    pub fn to_bytes(&self) -> Result<Vec<u8>, OxImageError> {
        serde_json::to_vec_pretty(self).map_err(OxImageError::Serialize)
    }

    /// Deserialize + validate + structurally verify an `.oxi` blob. Verification
    /// is a trust boundary: a corrupt or foreign `.oxi` with a valid header but
    /// out-of-range entry/func/block references would otherwise panic the host
    /// (an OOB index) when a frame is built, so every contained program is run
    /// through [`verify_program`](crate::verify::verify_program) here.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, OxImageError> {
        let image: Self = serde_json::from_slice(bytes).map_err(OxImageError::Deserialize)?;
        image.validate()?;
        for (index, program) in image.programs.iter().enumerate() {
            crate::verify::verify_program(program).map_err(|errors| {
                OxImageError::VerificationFailed {
                    program: index,
                    errors,
                }
            })?;
        }
        Ok(image)
    }

    /// Reject a stale/foreign blob, an empty image, or an out-of-range entry.
    pub fn validate(&self) -> Result<(), OxImageError> {
        if self.format != OX_IMAGE_FORMAT {
            return Err(OxImageError::UnknownFormat {
                format: self.format.clone(),
            });
        }
        if self.version != OX_IMAGE_VERSION {
            return Err(OxImageError::UnsupportedVersion {
                version: self.version,
            });
        }
        if self.programs.is_empty() {
            return Err(OxImageError::Empty);
        }
        if self.entry >= self.programs.len() {
            return Err(OxImageError::EntryOutOfRange {
                entry: self.entry,
                count: self.programs.len(),
            });
        }
        Ok(())
    }

    /// The program references in link order (entry last), for `Vm3::link`.
    pub fn program_refs(&self) -> Vec<&OxProgram> {
        self.programs.iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_bytes() {
        let image = OxImage::new(vec![OxProgram::default(), OxProgram::default()]);
        assert_eq!(image.entry, 1, "entry is the last program");
        let bytes = image.to_bytes().expect("serialize");
        let back = OxImage::from_bytes(&bytes).expect("deserialize");
        assert_eq!(image, back);
        assert_eq!(back.program_refs().len(), 2);
    }

    #[test]
    fn from_bytes_rejects_out_of_range_program_entry() {
        // A .oxi with a valid header but a program whose `entry` FuncId is out of
        // range used to pass load (only header/count/entry were checked) and then
        // panic when a frame was built. It must now be rejected at load.
        let program = OxProgram {
            entry: Some(crate::FuncId(5)), // no funcs -> out of range
            ..OxProgram::default()
        };
        let bytes = OxImage::new(vec![program]).to_bytes().expect("serialize");
        assert!(
            matches!(
                OxImage::from_bytes(&bytes),
                Err(OxImageError::VerificationFailed { program: 0, .. })
            ),
            "a corrupt program.entry must be rejected at load, not reach the runtime"
        );
    }

    #[test]
    fn rejects_empty_and_bad_header() {
        assert!(matches!(
            OxImage::new(Vec::new()).validate(),
            Err(OxImageError::Empty)
        ));
        let mut bad = OxImage::new(vec![OxProgram::default()]);
        bad.format = "nope".into();
        assert!(matches!(
            OxImage::from_bytes(&serde_json::to_vec(&bad).unwrap()),
            Err(OxImageError::UnknownFormat { .. })
        ));
    }
}
