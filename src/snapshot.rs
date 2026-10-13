//! A versioned projection boundary, not a PostgreSQL or Effect dependency.
use crate::{model::*, money::quantity, sha256, valid_hash, Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Year {
    pub ordinal: i32,
    pub start: String,
    pub end: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema: String,
    pub basis: SnapshotBasis,
    pub organization: Option<String>,
    pub organization_parts: Vec<String>,
    pub currency: String,
    pub chart_family: String,
    pub name: Option<String>,
    pub extent_date: Option<String>,
    pub years: Vec<Year>,
    pub accounts: Vec<Account>,
    pub dimensions: Vec<Dimension>,
    pub objects: Vec<Object>,
    pub balances: Vec<Balance>,
    pub vouchers: Vec<Voucher>,
}
pub fn parse(bytes: &[u8], l: &Limits) -> Result<Document> {
    if bytes.len() > l.bytes {
        return Err(Error::new("LIMIT_BYTES", "Snapshot exceeds byte limit"));
    }
    let s: Snapshot = crate::strict_json::from_slice(bytes)?;
    if s.schema != "siediff.snapshot.v1"
        || !matches!(
            s.basis.projection.as_str(),
            "posted_effects_v1" | "posted_with_sie_history_v1"
        )
    {
        return Err(Error::new(
            "SNAPSHOT_VERSION",
            "Unsupported snapshot/projection version",
        ));
    }
    if s.basis.book_id.is_empty()
        || s.basis.revision.is_empty()
        || s.basis.captured_at.is_empty()
        || s.basis.source_artifacts.iter().any(|s| !valid_hash(s))
    {
        return Err(Error::new(
            "SNAPSHOT_BASIS",
            "Snapshot lacks a revision-bound basis",
        ));
    }
    if s.currency.len() != 3 || !s.currency.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err(Error::new("CURRENCY", "Invalid reporting currency"));
    }
    let mut years = BTreeMap::new();
    for y in s.years {
        if y.ordinal > 0
            || years
                .insert(y.ordinal, Period::new(&y.start, &y.end)?)
                .is_some()
        {
            return Err(Error::new(
                "SNAPSHOT_YEAR",
                "Repeated or invalid fiscal ordinal",
            ));
        }
    }
    let mut accounts = BTreeMap::new();
    for a in s.accounts {
        crate::parse::numeric(&a.number)?;
        if a.kind
            .as_deref()
            .is_some_and(|k| !matches!(k, "T" | "S" | "K" | "I"))
        {
            return Err(Error::new("ACCOUNT_KIND", "Invalid account type"));
        }
        if accounts.insert(a.number.clone(), a).is_some() {
            return Err(Error::new("SNAPSHOT_ACCOUNT", "Repeated account"));
        }
    }
    let mut dimensions = BTreeMap::new();
    for d in s.dimensions {
        crate::parse::numeric(&d.id)?;
        if dimensions.insert(d.id.clone(), d).is_some() {
            return Err(Error::new("SNAPSHOT_DIMENSION", "Repeated dimension"));
        }
    }
    let mut d = Document {
        sha256: sha256(bytes),
        byte_length: bytes.len(),
        encoding: Encoding::Utf8,
        sie_type: 4,
        type_defaulted: false,
        organization: s.organization,
        organization_parts: s.organization_parts,
        currency: s.currency,
        currency_defaulted: false,
        name: s.name,
        extent_date: s.extent_date,
        chart_family: s.chart_family,
        chart_family_defaulted: false,
        years,
        accounts,
        dimensions,
        objects: s.objects,
        balances: s.balances,
        vouchers: s.vouchers,
        records: vec![],
        unsupported: vec![],
        checksum: Checksum::Absent,
        snapshot: Some(s.basis),
    };
    let mut n = 0;
    let loc = |n| Location {
        line: 0,
        byte_start: 0,
        byte_end: bytes.len(),
        record: n,
    };
    for b in &mut d.balances {
        b.at = loc(n);
        n += 1;
        b.objects.sort();
        b.quantity = quantity(b.quantity.as_deref().unwrap_or(""))?;
    }
    for v in &mut d.vouchers {
        v.at = loc(n);
        n += 1;
        for r in &mut v.rows {
            r.at = loc(n);
            n += 1;
            r.objects.sort();
            r.quantity = quantity(r.quantity.as_deref().unwrap_or(""))?;
        }
    }
    if d.snapshot
        .as_ref()
        .is_some_and(|s| s.projection == "posted_effects_v1")
        && d.vouchers
            .iter()
            .flat_map(|v| &v.rows)
            .any(|r| r.kind != RowKind::Final)
    {
        return Err(Error::new(
            "SNAPSHOT_HISTORY",
            "Posted-effects projection contains historical row kinds",
        ));
    }
    crate::validate::ensure_bounded(&d, l)?;
    crate::relationships::check(&d, l)?;
    Ok(d)
}
