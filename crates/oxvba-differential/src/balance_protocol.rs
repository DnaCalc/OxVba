//! Versioned subprocess protocol for fixture-addressable carrier-balance evidence.
//!
//! Runtime live counters are process-global. A report is therefore meaningful only
//! when one named fixture owns the child process that takes the before/after samples.
//! The parent harness may run many such children concurrently without their counters
//! contaminating one another.

use std::collections::BTreeMap;

use oxvba_runtime::HandleBalance;
use serde::{Deserialize, Serialize};

use crate::{Canon, RunOutcome};

pub const BALANCE_PROTOCOL_SCHEMA: &str = "oxvba.balance-fixture/v1";
pub const BALANCE_PROTOCOL_LINE_PREFIX: &str = "OXVBA_BALANCE_V1\t";
pub const MAX_BALANCE_PROTOCOL_BYTES: usize = 64 * 1024;

pub const CLEAN_BALANCE_FIXTURES: &[&str] = &[
    "carrier-string",
    "carrier-array",
    "carrier-object",
    "carrier-record",
];

pub const POLICY_ERROR_BALANCE_FIXTURE: &str = "host-policy-error";

pub const ALL_BALANCE_FIXTURES: &[&str] = &[
    "carrier-string",
    "carrier-array",
    "carrier-object",
    "carrier-record",
    POLICY_ERROR_BALANCE_FIXTURE,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureCompletion {
    Completed,
    Raised,
    Unsupported,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureResultObservation {
    pub completion: FixtureCompletion,
    pub values: Vec<CanonObservation>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CanonObservation {
    Empty,
    Null,
    Bool {
        value: bool,
    },
    Single {
        bits: u32,
    },
    Float {
        tag: u16,
        bits: u64,
    },
    Raw {
        tag: u16,
        bytes: [u8; 8],
        reserved: [u16; 3],
    },
    String {
        value: String,
    },
    Opaque {
        tag: u16,
    },
}

impl From<&Canon> for CanonObservation {
    fn from(value: &Canon) -> Self {
        match value {
            Canon::Empty => Self::Empty,
            Canon::Null => Self::Null,
            Canon::Bool(value) => Self::Bool { value: *value },
            Canon::Single(bits) => Self::Single { bits: *bits },
            Canon::Float { tag, bits } => Self::Float {
                tag: *tag,
                bits: *bits,
            },
            Canon::Raw {
                tag,
                bytes,
                reserved,
            } => Self::Raw {
                tag: *tag,
                bytes: *bytes,
                reserved: *reserved,
            },
            Canon::Str(value) => Self::String {
                value: value.clone(),
            },
            Canon::Opaque { tag } => Self::Opaque { tag: *tag },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullErrObservation {
    pub number: i32,
    pub source: String,
    pub description: String,
    pub last_dll_error: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarrierDeltas {
    pub bstrs: i64,
    pub object_boxes: i64,
    pub safearrays: i64,
    pub record_buffers: i64,
    /// Reserved named counters allow later carrier choke points to join the
    /// protocol without making anonymous positional fields.
    pub related: BTreeMap<String, i64>,
}

impl CarrierDeltas {
    pub fn is_zero(&self) -> bool {
        self.bstrs == 0
            && self.object_boxes == 0
            && self.safearrays == 0
            && self.record_buffers == 0
            && self.related.values().all(|delta| *delta == 0)
    }
}

impl TryFrom<HandleBalance> for CarrierDeltas {
    type Error = String;

    fn try_from(value: HandleBalance) -> Result<Self, Self::Error> {
        let fixed_width = |name: &str, delta: isize| {
            i64::try_from(delta)
                .map_err(|_| format!("carrier delta `{name}` does not fit protocol i64"))
        };
        Ok(Self {
            bstrs: fixed_width("bstrs", value.bstrs)?,
            object_boxes: fixed_width("object_boxes", value.object_boxes)?,
            safearrays: fixed_width("safearrays", value.safearrays)?,
            record_buffers: fixed_width("record_buffers", value.record_buffers)?,
            related: BTreeMap::new(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BalanceFixtureReport {
    pub schema: String,
    pub fixture: String,
    pub executor: String,
    pub result: FixtureResultObservation,
    pub full_err: FullErrObservation,
    /// Process-global carrier deltas for the dedicated fixture child.
    pub carrier_deltas: CarrierDeltas,
}

impl BalanceFixtureReport {
    /// Build a fixture report from a synchronous run and the dedicated child's
    /// independently sampled process-global carrier balance.
    ///
    /// `RunOutcome::handle_balance` remains the current-runner-thread
    /// observable. It must exist and be clean so a future cross-thread runtime
    /// path cannot silently weaken this synchronous fixture contract. The
    /// serialized `carrier_deltas` come only from `process_handle_balance`.
    pub fn from_process_balanced_outcome(
        fixture: impl Into<String>,
        outcome: RunOutcome,
        process_handle_balance: HandleBalance,
    ) -> Result<Self, String> {
        let runner_thread_balance = outcome.handle_balance.ok_or_else(|| {
            "fixture outcome has no runner-thread balance measurement".to_string()
        })?;
        if !runner_thread_balance.is_zero() {
            return Err(format!(
                "fixture outcome has a runner-thread carrier imbalance: {runner_thread_balance:?}"
            ));
        }
        let carrier_deltas = process_handle_balance.try_into()?;
        let result = if let Some(what) = outcome.unsupported {
            FixtureResultObservation {
                completion: FixtureCompletion::Unsupported,
                values: Vec::new(),
                message: Some(what),
            }
        } else {
            match outcome.result {
                Ok(values) => FixtureResultObservation {
                    completion: FixtureCompletion::Completed,
                    values: values.iter().map(CanonObservation::from).collect(),
                    message: None,
                },
                Err(message) => FixtureResultObservation {
                    completion: if outcome.raised {
                        FixtureCompletion::Raised
                    } else {
                        FixtureCompletion::Failed
                    },
                    values: Vec::new(),
                    message: Some(message),
                },
            }
        };
        let full_err = FullErrObservation {
            number: outcome.err.number,
            source: outcome.err.source,
            description: outcome.err.description,
            last_dll_error: outcome.err.last_dll_error,
        };
        Ok(Self {
            schema: BALANCE_PROTOCOL_SCHEMA.to_string(),
            fixture: fixture.into(),
            executor: "vm3".to_string(),
            result,
            full_err,
            carrier_deltas,
        })
    }

    pub fn to_protocol_line(&self) -> Result<String, String> {
        serde_json::to_string(self)
            .map(|json| format!("{BALANCE_PROTOCOL_LINE_PREFIX}{json}"))
            .map_err(|err| format!("failed to encode balance report: {err}"))
    }

    pub fn parse_protocol_output(output: &str) -> Result<Self, String> {
        if output.len() > MAX_BALANCE_PROTOCOL_BYTES {
            return Err(format!(
                "balance child output exceeds {MAX_BALANCE_PROTOCOL_BYTES} bytes"
            ));
        }
        let mut payloads = output
            .lines()
            .filter_map(|line| line.strip_prefix(BALANCE_PROTOCOL_LINE_PREFIX));
        let payload = payloads
            .next()
            .ok_or_else(|| "balance child emitted no protocol line".to_string())?;
        if payloads.next().is_some() {
            return Err("balance child emitted more than one protocol line".to_string());
        }
        let report: Self = serde_json::from_str(payload)
            .map_err(|err| format!("invalid balance protocol JSON: {err}"))?;
        report.validate()?;
        Ok(report)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != BALANCE_PROTOCOL_SCHEMA {
            return Err(format!(
                "unsupported balance protocol schema `{}`",
                self.schema
            ));
        }
        if self.fixture.is_empty() {
            return Err("balance report fixture identity is empty".to_string());
        }
        if self.executor != "vm3" {
            return Err(format!(
                "unsupported balance report executor `{}`",
                self.executor
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_parser_requires_one_named_versioned_line() {
        let report = BalanceFixtureReport {
            schema: BALANCE_PROTOCOL_SCHEMA.to_string(),
            fixture: "fixture-a".to_string(),
            executor: "vm3".to_string(),
            result: FixtureResultObservation {
                completion: FixtureCompletion::Completed,
                values: vec![CanonObservation::Raw {
                    tag: 3,
                    bytes: [1, 0, 0, 0, 0, 0, 0, 0],
                    reserved: [0, 0, 0],
                }],
                message: None,
            },
            full_err: FullErrObservation {
                number: 0,
                source: String::new(),
                description: String::new(),
                last_dll_error: 0,
            },
            carrier_deltas: CarrierDeltas::default(),
        };
        let line = report.to_protocol_line().expect("encode protocol line");
        assert_eq!(
            BalanceFixtureReport::parse_protocol_output(&line).expect("parse protocol line"),
            report
        );
        assert!(BalanceFixtureReport::parse_protocol_output("ordinary child output").is_err());
        assert!(BalanceFixtureReport::parse_protocol_output(&format!("{line}\n{line}")).is_err());
        assert!(
            BalanceFixtureReport::parse_protocol_output(
                &"x".repeat(MAX_BALANCE_PROTOCOL_BYTES + 1)
            )
            .is_err()
        );

        let mut wrong_schema = report.clone();
        wrong_schema.schema = "oxvba.balance-fixture/v0".to_string();
        assert!(wrong_schema.validate().is_err());
        let mut wrong_executor = report.clone();
        wrong_executor.executor = "jit".to_string();
        assert!(wrong_executor.validate().is_err());
        let mut unnamed = report;
        unnamed.fixture.clear();
        assert!(unnamed.validate().is_err());
    }

    #[test]
    fn report_construction_rejects_a_missing_balance_measurement() {
        let outcome = RunOutcome {
            result: Ok(Vec::new()),
            err: oxvba_host::FinalErr::default(),
            raised: false,
            unsupported: None,
            handle_balance: None,
        };
        let err = BalanceFixtureReport::from_process_balanced_outcome(
            "fixture-a",
            outcome,
            HandleBalance::default(),
        )
        .expect_err("missing measurement must fail closed");
        assert!(
            err.contains("no runner-thread balance measurement"),
            "{err}"
        );
    }

    #[test]
    fn report_construction_rejects_a_runner_thread_imbalance() {
        let outcome = RunOutcome {
            result: Ok(Vec::new()),
            err: oxvba_host::FinalErr::default(),
            raised: false,
            unsupported: None,
            handle_balance: Some(HandleBalance {
                bstrs: 1,
                ..HandleBalance::default()
            }),
        };
        let err = BalanceFixtureReport::from_process_balanced_outcome(
            "fixture-a",
            outcome,
            HandleBalance::default(),
        )
        .expect_err("runner-thread imbalance must fail closed");
        assert!(err.contains("runner-thread carrier imbalance"), "{err}");
    }
}
