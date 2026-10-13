use crate::{Error, Result};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
/// Signed hundredths. JSON always uses an integer STRING, never an IEEE number.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Money(pub i128);
impl Money {
    pub const ZERO: Self = Self(0);
    pub fn major(s: &str) -> Result<Self> {
        let (neg, u) = s.strip_prefix('-').map_or((false, s), |x| (true, x));
        let mut p = u.split('.');
        let whole = p.next().unwrap_or("");
        let frac = p.next();
        if whole.is_empty()
            || !whole.bytes().all(|c| c.is_ascii_digit())
            || p.next().is_some()
            || frac.is_some_and(|s| {
                s.is_empty() || s.len() > 2 || !s.bytes().all(|c| c.is_ascii_digit())
            })
        {
            return Err(Error::new(
                "AMOUNT_SYNTAX",
                "Expected decimal with at most two places",
            ));
        }
        // Negative accumulation supports the full i128 range without abs(MIN).
        let mut n = 0i128;
        for c in whole.bytes().chain(frac.unwrap_or("").bytes()) {
            n = n
                .checked_mul(10)
                .and_then(|n| n.checked_sub((c - b'0') as i128))
                .ok_or_else(|| Error::new("AMOUNT_OVERFLOW", "Minor-unit range exceeded"))?;
        }
        for _ in frac.map_or(0, str::len)..2 {
            n = n
                .checked_mul(10)
                .ok_or_else(|| Error::new("AMOUNT_OVERFLOW", "Minor-unit range exceeded"))?;
        }
        if !neg {
            n = n
                .checked_neg()
                .ok_or_else(|| Error::new("AMOUNT_OVERFLOW", "Minor-unit range exceeded"))?;
        }
        Ok(Self(n))
    }
    pub fn minor(s: &str) -> Result<Self> {
        let u = s.strip_prefix('-').unwrap_or(s);
        if u.is_empty() || !u.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Error::new(
                "MINOR_SYNTAX",
                "Minor units require an integer string",
            ));
        }
        s.parse::<i128>()
            .map(Self)
            .map_err(|_| Error::new("AMOUNT_OVERFLOW", "Minor-unit range exceeded"))
    }
    pub fn checked_add(self, b: Self) -> Result<Self> {
        self.0
            .checked_add(b.0)
            .map(Self)
            .ok_or_else(|| Error::new("SUM_OVERFLOW", "Exact sum exceeds i128"))
    }
    pub fn checked_sub(self, b: Self) -> Result<Self> {
        self.0
            .checked_sub(b.0)
            .map(Self)
            .ok_or_else(|| Error::new("SUM_OVERFLOW", "Exact difference exceeds i128"))
    }
    pub fn decimal(self) -> String {
        let u = self.0.unsigned_abs();
        format!(
            "{}{}.{:02}",
            if self.0 < 0 { "-" } else { "" },
            u / 100,
            u % 100
        )
    }
}
impl Serialize for Money {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.0.to_string())
    }
}
impl<'de> Deserialize<'de> for Money {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        Self::minor(&String::deserialize(d)?).map_err(de::Error::custom)
    }
}
/// Quantities are decimal scalars, not money. Do not round them to two places.
pub fn quantity(s: &str) -> Result<Option<String>> {
    if s.is_empty() {
        return Ok(None);
    }
    let (neg, u) = s.strip_prefix('-').map_or((false, s), |x| (true, x));
    let p: Vec<_> = u.split('.').collect();
    if p.len() > 2
        || p[0].len() > 38
        || p.iter()
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
        || p.get(1).is_some_and(|p| p.len() > 12)
    {
        return Err(Error::new(
            "QUANTITY_SYNTAX",
            "Quantity exceeds exact decimal profile",
        ));
    }
    let w = p[0].trim_start_matches('0');
    let w = if w.is_empty() { "0" } else { w };
    let f = p.get(1).copied().unwrap_or("").trim_end_matches('0');
    Ok(Some(format!(
        "{}{}{}{}",
        if neg && (w != "0" || !f.is_empty()) {
            "-"
        } else {
            ""
        },
        w,
        if f.is_empty() { "" } else { "." },
        f
    )))
}
