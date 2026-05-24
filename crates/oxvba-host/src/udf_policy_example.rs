use oxvba_compiler::{ProcedureDescriptor, ProcedureKind, ProjectReflection, VbaType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UdfAdmissionPolicy {
    pub allow_option_private_modules: bool,
    pub allowed_scalar_types: Vec<VbaType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedUdf {
    pub registration: W093RegistrationRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedUdfCandidate {
    pub callable_id: String,
    pub procedure_name: String,
    pub reason_code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UdfAdmissionReport {
    pub admitted: Vec<AdmittedUdf>,
    pub rejected: Vec<RejectedUdfCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W093RegistrationRequest {
    pub source_identity: W093SourceIdentity,
    pub callable_metadata: W093CallableMetadata,
    pub invocation_target: W093InvocationTarget,
    pub capability: W093Capability,
    pub change_facts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W093SourceIdentity {
    pub project_id: String,
    pub module_id: String,
    pub callable_id: String,
    pub descriptor_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W093CallableMetadata {
    pub public_name: String,
    pub module_name: String,
    pub parameter_count: usize,
    pub parameter_type_text: Vec<Option<String>>,
    pub return_type_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W093InvocationTarget {
    pub callable_id: String,
    pub conversion_lane: String,
    pub diagnostic_lane: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W093Capability {
    pub supported_value_subsets: Vec<String>,
    pub policy_name: String,
}

impl Default for UdfAdmissionPolicy {
    fn default() -> Self {
        Self {
            allow_option_private_modules: false,
            allowed_scalar_types: vec![VbaType::Long, VbaType::Double, VbaType::String],
        }
    }
}

impl UdfAdmissionPolicy {
    pub fn admit(&self, reflection: &ProjectReflection) -> UdfAdmissionReport {
        let mut admitted = Vec::new();
        let mut rejected = Vec::new();
        for procedure in &reflection.procedures {
            match self.project_candidate(procedure) {
                Ok(registration) => admitted.push(AdmittedUdf { registration }),
                Err(reason) => rejected.push(RejectedUdfCandidate {
                    callable_id: procedure.callable_id.clone(),
                    procedure_name: procedure.procedure_name.clone(),
                    reason_code: reason.0,
                    message: reason.1,
                }),
            }
        }
        UdfAdmissionReport { admitted, rejected }
    }

    fn project_candidate(
        &self,
        procedure: &ProcedureDescriptor,
    ) -> Result<W093RegistrationRequest, (String, String)> {
        if procedure.kind != ProcedureKind::Function {
            return Err((
                "POLICY-NOT-FUNCTION".to_string(),
                "only Functions are admitted".to_string(),
            ));
        }
        if !procedure.visibility.is_public {
            return Err((
                "POLICY-NOT-PUBLIC".to_string(),
                "private procedures are not admitted".to_string(),
            ));
        }
        if procedure.visibility.is_class_member {
            return Err((
                "POLICY-CLASS-MEMBER".to_string(),
                "class members require an object binding policy".to_string(),
            ));
        }
        if procedure.visibility.is_option_private && !self.allow_option_private_modules {
            return Err((
                "POLICY-OPTION-PRIVATE".to_string(),
                "Option Private modules are excluded by host policy".to_string(),
            ));
        }
        let return_type = procedure.signature.return_type.as_ref().ok_or_else(|| {
            (
                "POLICY-NO-RETURN".to_string(),
                "Function has no return descriptor".to_string(),
            )
        })?;
        if !self.allowed_scalar_types.contains(&return_type.normalized) {
            return Err((
                "POLICY-RETURN-TYPE".to_string(),
                "return type is not admitted by host policy".to_string(),
            ));
        }
        for param in &procedure.signature.parameters {
            let Some(value_type) = &param.value_type else {
                return Err((
                    "POLICY-PARAM-TYPE".to_string(),
                    "parameter type is unknown".to_string(),
                ));
            };
            if !self.allowed_scalar_types.contains(&value_type.normalized) {
                return Err((
                    "POLICY-PARAM-TYPE".to_string(),
                    "parameter type is not admitted by host policy".to_string(),
                ));
            }
        }
        Ok(W093RegistrationRequest {
            source_identity: W093SourceIdentity {
                project_id: procedure.project_id.clone(),
                module_id: procedure.module_id.clone(),
                callable_id: procedure.callable_id.clone(),
                descriptor_fingerprint: procedure.descriptor_fingerprint.clone(),
            },
            callable_metadata: W093CallableMetadata {
                public_name: procedure.procedure_name.clone(),
                module_name: procedure.module_name.clone(),
                parameter_count: procedure.signature.parameters.len(),
                parameter_type_text: procedure
                    .signature
                    .parameters
                    .iter()
                    .map(|param| param.source_type_text.clone())
                    .collect(),
                return_type_text: return_type.source_text.clone(),
            },
            invocation_target: W093InvocationTarget {
                callable_id: procedure.callable_id.clone(),
                conversion_lane: "TypedScalarFirstTier".to_string(),
                diagnostic_lane: "HostDiagnostic".to_string(),
            },
            capability: W093Capability {
                supported_value_subsets: vec!["typed-scalar-first-tier".to_string()],
                policy_name: "example-host-owned-udf-admission".to_string(),
            },
            change_facts: vec![
                "project-reflection-fingerprint".to_string(),
                "host-udf-policy".to_string(),
                "descriptor-fingerprint".to_string(),
            ],
        })
    }
}
