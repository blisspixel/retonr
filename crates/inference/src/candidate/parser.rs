use std::{cell::Cell, fmt};

use serde::de::{self, DeserializeSeed as _, MapAccess, SeqAccess, Visitor};

use super::{CandidateOutputError, CandidateOutputPolicy};
use crate::GenerationCandidate;

pub(super) fn parse(
    bytes: &[u8],
    policy: CandidateOutputPolicy,
) -> Result<Vec<GenerationCandidate>, CandidateOutputError> {
    let policy = CandidateOutputPolicy::new(
        policy.expected_count,
        policy.maximum_candidate_bytes,
        policy.maximum_aggregate_candidate_bytes,
    )?;
    let byte_count = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if byte_count > policy.maximum_envelope_bytes {
        return Err(CandidateOutputError::EnvelopeTooLarge);
    }
    let input = std::str::from_utf8(bytes).map_err(|_error| CandidateOutputError::InvalidUtf8)?;
    let captured_error = Cell::new(None);
    let seed = CandidateEnvelopeSeed {
        policy,
        captured_error: &captured_error,
    };
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let candidates = seed.deserialize(&mut deserializer).map_err(|_error| {
        captured_error
            .get()
            .unwrap_or(CandidateOutputError::InvalidEnvelope)
    })?;
    deserializer
        .end()
        .map_err(|_error| CandidateOutputError::InvalidEnvelope)?;
    if candidates.len() != usize::from(policy.expected_count) {
        return Err(CandidateOutputError::CountMismatch);
    }
    Ok(candidates)
}

struct CandidateEnvelopeSeed<'policy> {
    policy: CandidateOutputPolicy,
    captured_error: &'policy Cell<Option<CandidateOutputError>>,
}

impl<'de> serde::de::DeserializeSeed<'de> for CandidateEnvelopeSeed<'_> {
    type Value = Vec<GenerationCandidate>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(CandidateEnvelopeVisitor {
            policy: self.policy,
            captured_error: self.captured_error,
        })
    }
}

struct CandidateEnvelopeVisitor<'policy> {
    policy: CandidateOutputPolicy,
    captured_error: &'policy Cell<Option<CandidateOutputError>>,
}

impl<'de> Visitor<'de> for CandidateEnvelopeVisitor<'_> {
    type Value = Vec<GenerationCandidate>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a candidate envelope with exactly one candidates field")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let Some(CandidateEnvelopeField::Candidates) = map.next_key()? else {
            return Err(de::Error::missing_field("candidates"));
        };
        let candidates = map.next_value_seed(CandidateSequenceSeed {
            policy: self.policy,
            captured_error: self.captured_error,
        })?;
        if map.next_key::<CandidateEnvelopeField>()?.is_some() {
            return Err(de::Error::duplicate_field("candidates"));
        }
        Ok(candidates)
    }
}

#[derive(serde::Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum CandidateEnvelopeField {
    Candidates,
}

struct CandidateSequenceSeed<'policy> {
    policy: CandidateOutputPolicy,
    captured_error: &'policy Cell<Option<CandidateOutputError>>,
}

impl<'de> serde::de::DeserializeSeed<'de> for CandidateSequenceSeed<'_> {
    type Value = Vec<GenerationCandidate>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(CandidateSequenceVisitor {
            policy: self.policy,
            captured_error: self.captured_error,
        })
    }
}

struct CandidateSequenceVisitor<'policy> {
    policy: CandidateOutputPolicy,
    captured_error: &'policy Cell<Option<CandidateOutputError>>,
}

impl<'de> Visitor<'de> for CandidateSequenceVisitor<'_> {
    type Value = Vec<GenerationCandidate>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded candidate array")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut candidates = Vec::with_capacity(usize::from(self.policy.expected_count));
        let mut aggregate_bytes = 0_u64;
        while let Some(candidate) = sequence.next_element_seed(CandidateItemSeed)? {
            if candidates.len() == usize::from(self.policy.expected_count) {
                return self.fail(CandidateOutputError::CountMismatch);
            }
            let byte_count = u64::try_from(candidate.len()).unwrap_or(u64::MAX);
            if byte_count > self.policy.maximum_candidate_bytes {
                return self.fail(CandidateOutputError::CandidateTooLarge);
            }
            let Ok(next_aggregate_bytes) =
                checked_aggregate_byte_count(aggregate_bytes, byte_count)
            else {
                return self.fail(CandidateOutputError::AggregateByteCountOverflow);
            };
            if next_aggregate_bytes > self.policy.maximum_aggregate_candidate_bytes {
                return self.fail(CandidateOutputError::AggregateTooLarge);
            }
            let ordinal = u8::try_from(candidates.len()).map_err(de::Error::custom)?;
            candidates.push(GenerationCandidate {
                ordinal,
                text: candidate,
            });
            aggregate_bytes = next_aggregate_bytes;
        }
        Ok(candidates)
    }
}

pub(super) fn checked_aggregate_byte_count(
    current: u64,
    next: u64,
) -> Result<u64, CandidateOutputError> {
    current
        .checked_add(next)
        .ok_or(CandidateOutputError::AggregateByteCountOverflow)
}

impl CandidateSequenceVisitor<'_> {
    fn fail<T, E>(&self, error: CandidateOutputError) -> Result<T, E>
    where
        E: de::Error,
    {
        self.captured_error.set(Some(error));
        Err(E::custom(error))
    }
}

struct CandidateItemSeed;

impl<'de> serde::de::DeserializeSeed<'de> for CandidateItemSeed {
    type Value = String;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(CandidateItemVisitor)
    }
}

struct CandidateItemVisitor;

impl<'de> Visitor<'de> for CandidateItemVisitor {
    type Value = String;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a candidate with exactly one text field")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let Some(CandidateItemField::Text) = map.next_key()? else {
            return Err(de::Error::missing_field("text"));
        };
        let text = map.next_value()?;
        if map.next_key::<CandidateItemField>()?.is_some() {
            return Err(de::Error::duplicate_field("text"));
        }
        Ok(text)
    }
}

#[derive(serde::Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum CandidateItemField {
    Text,
}
