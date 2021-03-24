use serde::Serializer;
use serde::{de, Deserialize};
use std::fmt;

struct JsonStringVisitor;

impl<'de> de::Visitor<'de> for JsonStringVisitor {
    type Value = Vec<u8>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(v.as_bytes().to_vec())
    }
}

pub fn deserialize_string_to_vec_u8<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: de::Deserializer<'de>,
{
    // use our visitor to deserialize an `ActualValue`
    deserializer.deserialize_any(JsonStringVisitor)
}

pub(crate) fn serialize_string_from_vec_u8<S>(x: &Vec<u8>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(String::from_utf8_lossy(x.as_slice()).as_ref())
}

pub fn deserialize_string_to_opt_vec_u8<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<u8>>, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct OptionVisitor;

    impl<'de> de::Visitor<'de> for OptionVisitor {
        type Value = Option<Vec<u8>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string")
        }
        fn visit_some<D>(self, d: D) -> Result<Self::Value, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            Ok(Some(d.deserialize_str(JsonStringVisitor)?))
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }
    // use our visitor to deserialize an `ActualValue`
    deserializer.deserialize_option(OptionVisitor)
}

pub(crate) fn serialize_string_from_opt_vec_u8<S>(
    x: &Option<Vec<u8>>,
    s: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match x {
        Some(v) => s.serialize_str(String::from_utf8_lossy(v.as_slice()).as_ref()),
        None => s.serialize_str(""),
    }
}
