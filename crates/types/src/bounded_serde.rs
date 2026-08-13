use std::{fmt, marker::PhantomData};

use serde::{
    Deserialize, Deserializer,
    de::{Error as _, IgnoredAny, SeqAccess, Visitor},
};

pub(crate) struct BoundedVec<T, const MAXIMUM: usize>(pub(crate) Vec<T>);

pub(crate) struct BoundedString<const MAXIMUM: usize>(pub(crate) String);

impl<'de, const MAXIMUM: usize> Deserialize<'de> for BoundedString<MAXIMUM> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BoundedStringVisitor<const MAXIMUM: usize>;

        impl<const MAXIMUM: usize> Visitor<'_> for BoundedStringVisitor<MAXIMUM> {
            type Value = BoundedString<MAXIMUM>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "a string with at most {MAXIMUM} bytes")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value.len() > MAXIMUM {
                    return Err(E::custom("bounded string exceeds its byte limit"));
                }
                Ok(BoundedString(value.to_owned()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value.len() > MAXIMUM {
                    return Err(E::custom("bounded string exceeds its byte limit"));
                }
                Ok(BoundedString(value))
            }
        }

        deserializer.deserialize_str(BoundedStringVisitor::<MAXIMUM>)
    }
}

impl<T, const MAXIMUM: usize> Default for BoundedVec<T, MAXIMUM> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<'de, T, const MAXIMUM: usize> Deserialize<'de> for BoundedVec<T, MAXIMUM>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BoundedVecVisitor<T, const MAXIMUM: usize>(PhantomData<T>);

        impl<'de, T, const MAXIMUM: usize> Visitor<'de> for BoundedVecVisitor<T, MAXIMUM>
        where
            T: Deserialize<'de>,
        {
            type Value = BoundedVec<T, MAXIMUM>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "a sequence with at most {MAXIMUM} elements")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(MAXIMUM));
                while values.len() < MAXIMUM {
                    let Some(value) = sequence.next_element()? else {
                        return Ok(BoundedVec(values));
                    };
                    values.push(value);
                }
                if sequence.next_element::<IgnoredAny>()?.is_some() {
                    return Err(A::Error::custom("bounded sequence exceeds its limit"));
                }
                Ok(BoundedVec(values))
            }
        }

        deserializer.deserialize_seq(BoundedVecVisitor::<T, MAXIMUM>(PhantomData))
    }
}
