use rewrite_model::GenerationRequestBindingId;
use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};

use crate::{GenerationRequest, ReasoningPolicy};

impl GenerationRequest {
    /// Returns a versioned canonical digest binding every current request field.
    ///
    /// The digest is an equality binding, not anonymization. Predictable source or
    /// prompt text can still be recovered through dictionary testing. Canonical
    /// fields are hashed incrementally without a second prompt-sized buffer.
    #[must_use]
    pub fn binding_digest(&self) -> Digest {
        let mut material = Sha256::new();
        material.update(b"retonr:generation-request:v1\0");
        append_u32(&mut material, self.schema_version);
        append_digest(&mut material, self.artifact_id.digest());
        append_digest(&mut material, &self.artifact_digest);
        append_bytes(&mut material, self.input.as_bytes());
        append_digest(&mut material, &self.output.schema_digest);
        append_bytes(&mut material, self.output.schema_json.as_bytes());
        material.update([self.candidate_count]);
        append_u64(&mut material, self.source_byte_count);
        append_u64(&mut material, self.source_byte_limit);
        append_u64(&mut material, self.input_byte_limit);
        append_u32(&mut material, self.context_token_limit);
        append_u32(&mut material, self.output_token_limit);
        append_u64(&mut material, self.candidate_byte_limit);
        material.update(self.sampling.temperature.to_bits().to_be_bytes());
        material.update(self.sampling.top_p.to_bits().to_be_bytes());
        append_optional_u64(&mut material, self.sampling.seed);
        material.update([match self.reasoning {
            ReasoningPolicy::Disabled => 0,
            ReasoningPolicy::Discard => 1,
        }]);
        finish_digest(material)
    }

    /// Returns the typed portable identity for this exact provider-neutral request.
    ///
    /// This is an inert equality binding. It does not establish that a backend
    /// received the request or grant generation authority.
    #[must_use]
    pub fn generation_request_binding_id(&self) -> GenerationRequestBindingId {
        GenerationRequestBindingId::from_derived_digest(self.binding_digest())
    }
}

fn append_bytes(output: &mut Sha256, value: &[u8]) {
    append_u64(output, value.len() as u64);
    output.update(value);
}

fn append_digest(output: &mut Sha256, value: &Digest) {
    output.update(value.as_str().as_bytes());
}

fn append_optional_u64(output: &mut Sha256, value: Option<u64>) {
    match value {
        Some(value) => {
            output.update([1]);
            append_u64(output, value);
        }
        None => output.update([0]),
    }
}

fn append_u32(output: &mut Sha256, value: u32) {
    output.update(value.to_be_bytes());
}

fn append_u64(output: &mut Sha256, value: u64) {
    output.update(value.to_be_bytes());
}

fn finish_digest(output: Sha256) -> Digest {
    Digest::from_sha256_hex(format!("{:x}", output.finalize()))
        .expect("SHA-256 output is canonical lowercase hexadecimal")
}

#[cfg(test)]
mod tests {
    use rewrite_model::ArtifactId;
    use rewrite_types::Digest;

    use crate::{
        GENERATION_REQUEST_SCHEMA_VERSION, GenerationRequest, OutputContract, ReasoningPolicy,
        SamplingParameters,
    };

    fn request() -> GenerationRequest {
        let artifact_digest = Digest::sha256(b"artifact");
        let schema_json = "{\"type\":\"object\"}".to_owned();
        GenerationRequest {
            schema_version: GENERATION_REQUEST_SCHEMA_VERSION,
            artifact_id: ArtifactId::from_digest(artifact_digest.clone()),
            artifact_digest,
            input: "bounded input".to_owned(),
            output: OutputContract {
                schema_digest: Digest::sha256(schema_json.as_bytes()),
                schema_json,
            },
            candidate_count: 2,
            source_byte_count: 13,
            source_byte_limit: 1_024,
            input_byte_limit: 2_048,
            context_token_limit: 4_096,
            output_token_limit: 256,
            candidate_byte_limit: 1_024,
            sampling: SamplingParameters {
                temperature: 0.25,
                top_p: 0.9,
                seed: Some(7),
            },
            reasoning: ReasoningPolicy::Disabled,
        }
    }

    #[test]
    fn generation_request_binding_has_a_known_vector_without_changing_json() {
        let request = request();
        assert_eq!(
            request.binding_digest().as_str(),
            "9efe6a9e4c7ad24a2f747f565ca2c5b8bd40efb5d8a2c05484dcf0c4c29aafc8"
        );
        assert_eq!(
            request.generation_request_binding_id().digest(),
            &request.binding_digest()
        );
        assert_eq!(
            serde_json::to_string(&request).expect("request serializes"),
            concat!(
                "{\"schema_version\":1,",
                "\"artifact_id\":\"c7c5c1d70c5dec4416ab6158afd0b223ef40c29b1dc1f97ed9428b94d4cadb1c\",",
                "\"artifact_digest\":\"c7c5c1d70c5dec4416ab6158afd0b223ef40c29b1dc1f97ed9428b94d4cadb1c\",",
                "\"input\":\"bounded input\",",
                "\"output\":{\"schema_digest\":\"a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0\",",
                "\"schema_json\":\"{\\\"type\\\":\\\"object\\\"}\"},",
                "\"candidate_count\":2,\"source_byte_count\":13,",
                "\"source_byte_limit\":1024,\"input_byte_limit\":2048,",
                "\"context_token_limit\":4096,\"output_token_limit\":256,",
                "\"candidate_byte_limit\":1024,",
                "\"sampling\":{\"temperature\":0.25,\"top_p\":0.9,\"seed\":7},",
                "\"reasoning\":\"disabled\"}"
            )
        );
    }

    #[test]
    fn every_generation_request_field_changes_the_binding() {
        let original = request();
        let original_digest = original.binding_digest();
        let mut variants = Vec::new();

        let mut value = original.clone();
        value.schema_version = 2;
        variants.push(value);
        let mut value = original.clone();
        value.artifact_id = ArtifactId::from_digest(Digest::sha256(b"other artifact id"));
        variants.push(value);
        let mut value = original.clone();
        value.artifact_digest = Digest::sha256(b"other artifact digest");
        variants.push(value);
        let mut value = original.clone();
        value.input.push('!');
        variants.push(value);
        let mut value = original.clone();
        value.output.schema_digest = Digest::sha256(b"other schema digest");
        variants.push(value);
        let mut value = original.clone();
        value.output.schema_json.push(' ');
        variants.push(value);
        let mut value = original.clone();
        value.candidate_count = 1;
        variants.push(value);
        let mut value = original.clone();
        value.source_byte_count += 1;
        variants.push(value);
        let mut value = original.clone();
        value.source_byte_limit += 1;
        variants.push(value);
        let mut value = original.clone();
        value.input_byte_limit += 1;
        variants.push(value);
        let mut value = original.clone();
        value.context_token_limit += 1;
        variants.push(value);
        let mut value = original.clone();
        value.output_token_limit += 1;
        variants.push(value);
        let mut value = original.clone();
        value.candidate_byte_limit += 1;
        variants.push(value);
        let mut value = original.clone();
        value.sampling.temperature = 0.5;
        variants.push(value);
        let mut value = original.clone();
        value.sampling.top_p = 0.8;
        variants.push(value);
        let mut value = original.clone();
        value.sampling.seed = None;
        variants.push(value);
        let mut value = original;
        value.reasoning = ReasoningPolicy::Discard;
        variants.push(value);

        assert!(
            variants
                .iter()
                .all(|value| value.binding_digest() != original_digest)
        );
    }
}
