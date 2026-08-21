use std::collections::BTreeSet;
use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};

pub(crate) fn validate_unique_json(bytes: &[u8]) -> Result<(), ()> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    UniqueValue.deserialize(&mut deserializer).map_err(|_| ())?;
    deserializer.end().map_err(|_| ())
}

struct UniqueValue;

impl<'de> DeserializeSeed<'de> for UniqueValue {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueVisitor)
    }
}

struct UniqueVisitor;

impl<'de> Visitor<'de> for UniqueVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("one unambiguous JSON value")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        UniqueValue.deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element_seed(UniqueValue)?.is_some() {}
        Ok(())
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(de::Error::custom("duplicate JSON member"));
            }
            map.next_value_seed(UniqueValue)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::validate_unique_json;

    #[test]
    fn detects_recursive_duplicates_and_trailing_input() {
        assert!(validate_unique_json(br#"{"a":[1,{"b":true}],"c":null}"#).is_ok());
        assert!(validate_unique_json(br#"[false,-1,1.5,"text",null]"#).is_ok());
        assert!(validate_unique_json(br#"{"a":{"b":1,"b":2}}"#).is_err());
        assert!(validate_unique_json(br#"{"a":1} false"#).is_err());
    }
}
