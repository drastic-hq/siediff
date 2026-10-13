//! Reject duplicate keys before typed deserialization. Policies must not depend
//! on the JSON library's last-key-wins behavior.
use crate::{Error, Result};
use serde::{
    de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::{Number, Value};
use std::{collections::BTreeSet, fmt};
struct Unique {
    has_float: bool,
}
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON with unique object keys")
            }
            fn visit_bool<E: de::Error>(self, _: bool) -> std::result::Result<Unique, E> {
                Ok(Unique { has_float: false })
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> std::result::Result<Unique, E> {
                Ok(Unique { has_float: false })
            }
            fn visit_u64<E: de::Error>(self, _: u64) -> std::result::Result<Unique, E> {
                Ok(Unique { has_float: false })
            }
            fn visit_f64<E: de::Error>(self, n: f64) -> std::result::Result<Unique, E> {
                Number::from_f64(n)
                    .map(|_| Unique { has_float: true })
                    .ok_or_else(|| E::custom("Non-finite number"))
            }
            fn visit_str<E: de::Error>(self, _: &str) -> std::result::Result<Unique, E> {
                Ok(Unique { has_float: false })
            }
            fn visit_string<E: de::Error>(self, _: String) -> std::result::Result<Unique, E> {
                Ok(Unique { has_float: false })
            }
            fn visit_none<E: de::Error>(self) -> std::result::Result<Unique, E> {
                Ok(Unique { has_float: false })
            }
            fn visit_unit<E: de::Error>(self) -> std::result::Result<Unique, E> {
                Ok(Unique { has_float: false })
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Unique, A::Error> {
                let mut has_float = false;
                while let Some(x) = a.next_element::<Unique>()? {
                    has_float |= x.has_float;
                }
                Ok(Unique { has_float })
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Unique, A::Error> {
                let mut keys = BTreeSet::new();
                let mut has_float = false;
                while let Some(k) = a.next_key::<String>()? {
                    if !keys.insert(k) {
                        return Err(de::Error::custom("Duplicate JSON key"));
                    }
                    has_float |= a.next_value::<Unique>()?.has_float;
                }
                Ok(Unique { has_float })
            }
        }
        d.deserialize_any(V)
    }
}
pub fn from_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let syntax_error = |_| {
        Error::new(
            "JSON_SYNTAX",
            "Invalid JSON, excessive nesting or duplicate object key",
        )
    };
    let value: Unique = serde_json::from_slice(bytes).map_err(syntax_error)?;
    let typed = if value.has_float {
        // Preserve Value's number normalization, including negative zero.
        let value = serde_json::from_slice::<Value>(bytes).map_err(syntax_error)?;
        serde_json::from_value(value)
    } else {
        serde_json::from_slice(bytes)
    };
    typed.map_err(|_| {
        Error::new(
            "JSON_SCHEMA",
            "Input does not satisfy the strict typed contract",
        )
    })
}
