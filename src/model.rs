use crate::{money::Money, Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Encoding {
    #[default]
    Cp437,
    Utf8,
    Windows1252,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub bytes: usize,
    pub line_bytes: usize,
    pub records: usize,
    pub vouchers: usize,
    pub rows_per_voucher: usize,
    pub fields: usize,
    pub findings: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: 64 * 1024 * 1024,
            line_bytes: 64 * 1024,
            records: 1_000_000,
            vouchers: 200_000,
            rows_per_voucher: 20_000,
            fields: 256,
            findings: 10_000,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub line: usize,
    pub byte_start: usize,
    pub byte_end: usize,
    pub record: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(untagged)]
pub enum Field {
    Text(String),
    Objects(Vec<String>),
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub tag: String,
    pub fields: Vec<Field>,
    pub at: Location,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Period {
    pub start: String,
    pub end: String,
}
impl Period {
    pub fn new(a: &str, b: &str) -> Result<Self> {
        date(a)?;
        date(b)?;
        if a > b {
            return Err(Error::new("PERIOD_ORDER", "Period ends before it starts"));
        }
        Ok(Self {
            start: a.into(),
            end: b.into(),
        })
    }
    pub fn contains(&self, d: &str) -> bool {
        d >= self.start.as_str() && d <= self.end.as_str()
    }
    pub fn overlaps(&self, b: &Self) -> bool {
        self.start <= b.end && b.start <= self.end
    }
}
pub fn date(s: &str) -> Result<()> {
    if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::new("DATE_SYNTAX", "Expected YYYYMMDD"));
    }
    let y: u32 = s[..4]
        .parse()
        .map_err(|_| Error::new("DATE_SYNTAX", "Invalid year"))?;
    let m: usize = s[4..6]
        .parse()
        .map_err(|_| Error::new("DATE_SYNTAX", "Invalid month"))?;
    let d: u32 = s[6..]
        .parse()
        .map_err(|_| Error::new("DATE_SYNTAX", "Invalid day"))?;
    let feb = if y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400)) {
        29
    } else {
        28
    };
    if y == 0
        || !(1..=12).contains(&m)
        || d == 0
        || d > [31, feb, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m - 1]
    {
        return Err(Error::new("DATE_RANGE", "Invalid Gregorian date"));
    }
    Ok(())
}
pub fn month(s: &str) -> Result<()> {
    if s.len() != 6 {
        return Err(Error::new("MONTH_SYNTAX", "Expected YYYYMM"));
    }
    date(&format!("{s}01"))
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub number: String,
    pub name: String,
    pub kind: Option<String>,
    pub unit: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Dimension {
    pub id: String,
    pub name: String,
    pub parent: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub dimension: String,
    pub object: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Object {
    pub dimension: String,
    pub number: String,
    pub name: String,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RowKind {
    Final,
    AddedHistory,
    RemovedHistory,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub kind: RowKind,
    pub account: String,
    pub objects: Vec<Assignment>,
    pub amount: Money,
    pub date: Option<String>,
    pub text: Option<String>,
    pub quantity: Option<String>,
    pub signature: Option<String>,
    pub at: Location,
}
impl Row {
    pub fn effective_date<'a>(&'a self, v: &'a Voucher) -> &'a str {
        self.date.as_deref().unwrap_or(&v.date)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Voucher {
    pub series: String,
    pub number: String,
    pub date: String,
    pub text: Option<String>,
    pub registered_on: Option<String>,
    pub signature: Option<String>,
    pub rows: Vec<Row>,
    pub at: Location,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum BalanceKind {
    Opening,
    Closing,
    Result,
    ObjectOpening,
    ObjectClosing,
    PeriodMovement,
    Budget,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Balance {
    pub kind: BalanceKind,
    pub year: i32,
    pub month: Option<String>,
    pub account: String,
    pub objects: Vec<Assignment>,
    pub amount: Money,
    pub quantity: Option<String>,
    pub at: Location,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Checksum {
    Absent,
    Verified { value: u32 },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SnapshotBasis {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_relationships: Option<crate::relationships::Relationships>,
    pub book_id: String,
    pub revision: String,
    pub captured_at: String,
    pub projection: String,
    pub source_artifacts: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub sha256: String,
    pub byte_length: usize,
    pub encoding: Encoding,
    pub sie_type: u8,
    pub type_defaulted: bool,
    pub organization: Option<String>,
    pub organization_parts: Vec<String>,
    pub currency: String,
    pub currency_defaulted: bool,
    pub name: Option<String>,
    pub extent_date: Option<String>,
    pub chart_family: String,
    pub chart_family_defaulted: bool,
    pub years: BTreeMap<i32, Period>,
    pub accounts: BTreeMap<String, Account>,
    pub dimensions: BTreeMap<String, Dimension>,
    pub objects: Vec<Object>,
    pub balances: Vec<Balance>,
    pub vouchers: Vec<Voucher>,
    pub records: Vec<Record>,
    pub unsupported: Vec<Location>,
    pub checksum: Checksum,
    pub snapshot: Option<SnapshotBasis>,
}
impl Document {
    pub fn year_for(&self, d: &str) -> Option<(&i32, &Period)> {
        let mut m = self.years.iter().filter(|(_, p)| p.contains(d));
        let f = m.next()?;
        if m.next().is_some() {
            None
        } else {
            Some(f)
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pass,
    Fail,
    Indeterminate,
    NotApplicable,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub code: String,
    pub status: Status,
    pub message: String,
    pub locations: Vec<Location>,
}
impl Finding {
    pub fn new(code: &str, status: Status, message: &str) -> Self {
        Self {
            code: code.into(),
            status,
            message: message.into(),
            locations: vec![],
        }
    }
    pub fn at(mut self, l: &Location) -> Self {
        self.locations.push(l.clone());
        self
    }
}
pub fn combine(i: impl IntoIterator<Item = Status>) -> Status {
    let mut any = false;
    let mut u = false;
    for s in i {
        match s {
            Status::Fail => return Status::Fail,
            Status::Indeterminate => u = true,
            Status::Pass => any = true,
            Status::NotApplicable => {}
        }
    }
    if u {
        Status::Indeterminate
    } else if any {
        Status::Pass
    } else {
        Status::NotApplicable
    }
}
