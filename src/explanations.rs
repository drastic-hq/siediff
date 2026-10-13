use crate::{
    compare::{Mode, Policy},
    model::*,
    relationships::identity,
    Result,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Explanation {
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub certainty: String,
    pub basis: String,
    pub left: Option<Location>,
    pub right: Option<Location>,
}
fn values(v: &Voucher, r: &Row) -> Result<Vec<String>> {
    Ok(vec![
        r.account.clone(),
        r.amount.0.to_string(),
        r.effective_date(v).into(),
        crate::digest(&r.objects)?,
        r.quantity.clone().unwrap_or_default(),
    ])
}
fn signature(values: &[String], excluded: Option<usize>) -> Vec<&str> {
    values
        .iter()
        .enumerate()
        .filter(|(i, _)| Some(*i) != excluded)
        .map(|(_, v)| v.as_str())
        .collect()
}
pub fn explain(
    a: &Document,
    b: &Document,
    p: &Policy,
    l: &Limits,
) -> Result<(Vec<Explanation>, bool)> {
    let mut out = vec![];
    let mut truncated = false;
    let mut emit = |field: &str,
                    before: Option<String>,
                    after: Option<String>,
                    certainty: &str,
                    basis: &str,
                    left: Option<Location>,
                    right: Option<Location>| {
        if out.len() < l.findings {
            out.push(Explanation {
                field: field.into(),
                before,
                after,
                certainty: certainty.into(),
                basis: basis.into(),
                left,
                right,
            });
        } else {
            truncated = true;
        }
    };
    if p.mapping.is_some() {
        emit("correspondence", None, None, "unresolved", "Field explanations require unmapped scoped identities; inspect mapped financial groups", None, None);
        return Ok((out, truncated));
    }
    let selected = |v: &&Voucher| p.period.as_ref().is_none_or(|p| p.contains(&v.date));
    let mut left = BTreeMap::<String, Vec<&Voucher>>::new();
    let mut right = BTreeMap::<String, Vec<&Voucher>>::new();
    for (d, target) in [(a, &mut left), (b, &mut right)] {
        for v in d.vouchers.iter().filter(selected) {
            if let Some(id) = identity(d, v) {
                target.entry(id).or_default().push(v);
            }
        }
    }
    if p.mode == Mode::Renumbered {
        let financial = |d: &Document| -> Result<BTreeMap<String, usize>> {
            let mut counts = BTreeMap::new();
            for v in d.vouchers.iter().filter(selected) {
                let mut rows = v
                    .rows
                    .iter()
                    .filter(|r| r.kind == RowKind::Final)
                    .map(|r| values(v, r))
                    .collect::<Result<Vec<_>>>()?;
                rows.sort();
                *counts
                    .entry(crate::digest(&(v.date.clone(), rows))?)
                    .or_insert(0) += 1;
            }
            Ok(counts)
        };
        if financial(a)?
            .values()
            .chain(financial(b)?.values())
            .any(|n| *n > 1)
        {
            emit(
                "correspondence",
                None,
                None,
                "unresolved",
                "Identical repeated financial vouchers have no unique pairing",
                None,
                None,
            );
        }
    }
    let ids: BTreeSet<_> = left.keys().chain(right.keys()).cloned().collect();
    for id in ids {
        match (
            left.get(&id).map(Vec::as_slice).unwrap_or(&[]),
            right.get(&id).map(Vec::as_slice).unwrap_or(&[]),
        ) {
            ([x], [y]) => {
                if x.date != y.date {
                    emit(
                        "voucher_date",
                        Some(x.date.clone()),
                        Some(y.date.clone()),
                        "scoped_identity",
                        &id,
                        Some(x.at.clone()),
                        Some(y.at.clone()),
                    );
                }
                let lx: Vec<_> = x
                    .rows
                    .iter()
                    .filter(|r| r.kind == RowKind::Final)
                    .map(|r| Ok((r, values(x, r)?)))
                    .collect::<Result<_>>()?;
                let ry: Vec<_> = y
                    .rows
                    .iter()
                    .filter(|r| r.kind == RowKind::Final)
                    .map(|r| Ok((r, values(y, r)?)))
                    .collect::<Result<_>>()?;
                let mut used_l = BTreeSet::new();
                let mut used_r = BTreeSet::new();
                let mut exact = BTreeMap::<Vec<&str>, Vec<usize>>::new();
                for (i, (_, v)) in ry.iter().enumerate() {
                    exact.entry(signature(v, None)).or_default().push(i);
                }
                for (i, (_, v)) in lx.iter().enumerate() {
                    if let Some(j) = exact.get_mut(&signature(v, None)).and_then(Vec::pop) {
                        used_l.insert(i);
                        used_r.insert(j);
                    }
                }
                for (field_index, field) in [
                    "account",
                    "amount",
                    "effective_date",
                    "dimensions",
                    "quantity",
                ]
                .iter()
                .enumerate()
                {
                    let mut la = BTreeMap::<Vec<&str>, Vec<usize>>::new();
                    let mut rb = BTreeMap::<Vec<&str>, Vec<usize>>::new();
                    for (i, (_, v)) in lx.iter().enumerate().filter(|(i, _)| !used_l.contains(i)) {
                        la.entry(signature(v, Some(field_index)))
                            .or_default()
                            .push(i);
                    }
                    for (i, (_, v)) in ry.iter().enumerate().filter(|(i, _)| !used_r.contains(i)) {
                        rb.entry(signature(v, Some(field_index)))
                            .or_default()
                            .push(i);
                    }
                    for (key, li) in la {
                        if let ([i], Some(rj)) = (li.as_slice(), rb.get(&key)) {
                            if let [j] = rj.as_slice() {
                                let before = if *field == "dimensions" {
                                    serde_json::to_string(&lx[*i].0.objects).unwrap_or_default()
                                } else {
                                    lx[*i].1[field_index].clone()
                                };
                                let after = if *field == "dimensions" {
                                    serde_json::to_string(&ry[*j].0.objects).unwrap_or_default()
                                } else {
                                    ry[*j].1[field_index].clone()
                                };
                                emit(
                                    field,
                                    Some(before),
                                    Some(after),
                                    "scoped_identity_single_field",
                                    &id,
                                    Some(lx[*i].0.at.clone()),
                                    Some(ry[*j].0.at.clone()),
                                );
                                used_l.insert(*i);
                                used_r.insert(*j);
                            }
                        }
                    }
                }
                if used_l.len() < lx.len() && used_r.len() < ry.len() {
                    emit(
                        "correspondence",
                        None,
                        None,
                        "unresolved",
                        &id,
                        Some(x.at.clone()),
                        Some(y.at.clone()),
                    );
                }
                for (i, (r, v)) in lx.iter().enumerate().filter(|(i, _)| !used_l.contains(i)) {
                    let _ = i;
                    emit(
                        "removed_row",
                        Some(serde_json::to_string(v).unwrap_or_default()),
                        None,
                        "unpaired",
                        &id,
                        Some(r.at.clone()),
                        None,
                    );
                }
                for (i, (r, v)) in ry.iter().enumerate().filter(|(i, _)| !used_r.contains(i)) {
                    let _ = i;
                    emit(
                        "added_row",
                        None,
                        Some(serde_json::to_string(v).unwrap_or_default()),
                        "unpaired",
                        &id,
                        None,
                        Some(r.at.clone()),
                    );
                }
            }
            ([x], []) => emit(
                "missing_voucher",
                Some(id.clone()),
                None,
                "identity_absence",
                "Unpaired scoped voucher; renumbering may explain identity absence",
                Some(x.at.clone()),
                None,
            ),
            ([], [y]) => emit(
                "added_voucher",
                None,
                Some(id.clone()),
                "identity_absence",
                "Unpaired scoped voucher; renumbering may explain identity absence",
                None,
                Some(y.at.clone()),
            ),
            _ => emit(
                "correspondence",
                Some(id),
                None,
                "unresolved",
                "Scoped identities are not unique",
                None,
                None,
            ),
        }
    }
    for (side, d) in [("left", a), ("right", b)] {
        for finding in crate::validate::validate(d, l)?
            .findings
            .into_iter()
            .filter(|f| f.status != Status::Pass)
        {
            emit(
                "control_failure",
                Some(finding.code),
                Some(finding.message),
                "observed_validation",
                side,
                finding.locations.first().cloned(),
                None,
            );
        }
    }
    Ok((out, truncated))
}
