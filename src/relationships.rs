use crate::{control::Coverage, model::*, Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub voucher: String,
    pub corrects: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Relationships {
    pub coverage: Coverage,
    pub links: Vec<Link>,
}
pub fn identity(d: &Document, v: &Voucher) -> Option<String> {
    let period = d.years.values().find(|p| p.contains(&v.date))?;
    Some(format!(
        "{}:{}:{}:{}",
        period.start, period.end, v.series, v.number
    ))
}
pub fn check(d: &Document, l: &Limits) -> Result<()> {
    let Some(relationships) = d
        .snapshot
        .as_ref()
        .and_then(|s| s.native_relationships.as_ref())
    else {
        return Ok(());
    };
    if relationships.links.len() > l.vouchers {
        return Err(Error::new(
            "LIMIT_RELATIONSHIPS",
            "Too many correction links",
        ));
    }
    let identities: BTreeSet<_> = d.vouchers.iter().filter_map(|v| identity(d, v)).collect();
    if identities.len() != d.vouchers.len() {
        return Err(Error::new(
            "NATIVE_IDENTITY",
            "Correction membership requires unique scoped identities",
        ));
    }
    let mut edges = BTreeMap::new();
    for link in &relationships.links {
        if !identities.contains(&link.voucher)
            || !identities.contains(&link.corrects)
            || edges
                .insert(link.voucher.as_str(), link.corrects.as_str())
                .is_some()
        {
            return Err(Error::new(
                "NATIVE_RELATIONSHIP",
                "Dangling or repeated correction relationship",
            ));
        }
    }
    let mut done = BTreeSet::new();
    for start in edges.keys() {
        let mut path = BTreeSet::new();
        let mut current = *start;
        while !done.contains(current) {
            if !path.insert(current) {
                return Err(Error::new(
                    "NATIVE_CYCLE",
                    "Correction graph contains a cycle",
                ));
            }
            let Some(next) = edges.get(current) else {
                break;
            };
            current = next;
        }
        done.extend(path);
    }
    Ok(())
}
pub fn status(a: &Document, b: &Document) -> Status {
    match (
        a.snapshot
            .as_ref()
            .and_then(|s| s.native_relationships.as_ref()),
        b.snapshot
            .as_ref()
            .and_then(|s| s.native_relationships.as_ref()),
    ) {
        (Some(a), Some(b))
            if a.coverage == Coverage::CompleteAsserted
                && b.coverage == Coverage::CompleteAsserted =>
        {
            let a: BTreeSet<_> = a.links.iter().collect();
            let b: BTreeSet<_> = b.links.iter().collect();
            if a == b {
                Status::Pass
            } else {
                Status::Fail
            }
        }
        _ => Status::Indeterminate,
    }
}
