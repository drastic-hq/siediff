use crate::{model::*, money::Money, parse, Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validation {
    pub schema: String,
    pub status: Status,
    pub findings: Vec<Finding>,
    pub findings_truncated: bool,
    pub totals: Totals,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Totals {
    pub vouchers: usize,
    pub final_rows: usize,
    pub history_rows: usize,
    pub final_sum: Money,
}
fn push(f: &mut Vec<Finding>, truncated: &mut bool, max: usize, v: Finding) {
    if f.len() < max {
        f.push(v)
    } else {
        *truncated = true
    }
}
pub fn same_row(a: &Row, b: &Row) -> bool {
    a.account == b.account
        && a.objects == b.objects
        && a.amount == b.amount
        && a.date == b.date
        && a.text == b.text
        && a.quantity == b.quantity
        && a.signature == b.signature
}
pub fn validate(d: &Document, l: &Limits) -> Result<Validation> {
    ensure_bounded(d, l)?;
    let mut f = vec![];
    let mut cut = false;
    let mut status = Status::Pass;
    let mut emit = |x: Finding| {
        status = combine([status, x.status]);
        push(&mut f, &mut cut, l.findings, x)
    };
    if d.snapshot.is_none() {
        if d.records.first().map(|r| r.tag.as_str()) != Some("#FLAGGA") {
            emit(Finding::new(
                "FIRST_FLAG",
                Status::Fail,
                "FLAGGA is not the first retained record",
            ));
        }
        for tag in ["#FLAGGA", "#PROGRAM", "#FORMAT", "#GEN", "#FNAMN"] {
            if !d.records.iter().any(|r| r.tag == tag) {
                emit(Finding::new(
                    "REQUIRED_HEADER",
                    Status::Fail,
                    "A required identification header is absent",
                ));
            }
        }
        if let Some(r) = d.records.iter().find(|r| r.tag == "#FORMAT") {
            if parse::text(r, 0)? != "PC8" {
                emit(
                    Finding::new(
                        "NONSTANDARD_FORMAT",
                        Status::Indeterminate,
                        "Declared character set is outside the PC8 profile",
                    )
                    .at(&r.at),
                );
            }
        }
        if d.encoding != Encoding::Cp437 {
            emit(Finding::new(
                "NONSTANDARD_ENCODING",
                Status::Indeterminate,
                "Explicit non-PC8 decoding is diagnostic, not standard conformance",
            ));
        }
    }
    if !(1..=4).contains(&d.sie_type) {
        emit(Finding::new(
            "SIE_TYPE",
            Status::Fail,
            "Unsupported SIE type",
        ));
    }
    if !d.unsupported.is_empty() {
        emit(Finding {
            code: "UNINTERPRETED_RECORDS".into(),
            status: Status::Indeterminate,
            message: "Uninterpreted source records may carry additional meaning".into(),
            locations: d.unsupported.iter().take(10).cloned().collect(),
        });
    }
    if !matches!(
        d.chart_family.as_str(),
        "BAS95" | "BAS96" | "EUBAS97" | "NE2007"
    ) {
        emit(Finding::new(
            "CHART_FAMILY",
            Status::Indeterminate,
            "Chart family is outside the recognized SIE profile",
        ));
    }
    let periods: Vec<_> = d.years.iter().collect();
    for (i, (ya, a)) in periods.iter().enumerate() {
        Period::new(&a.start, &a.end)?;
        for (yb, b) in periods.iter().skip(i + 1) {
            if a.overlaps(b) {
                emit(Finding::new(
                    "YEAR_OVERLAP",
                    Status::Fail,
                    "Declared fiscal intervals overlap",
                ));
            }
            if ya < yb && a.end >= b.start {
                emit(Finding::new(
                    "YEAR_ORDINAL_ORDER",
                    Status::Fail,
                    "Fiscal ordinals disagree with chronological intervals",
                ));
            }
        }
    }
    if d.years.is_empty() {
        emit(Finding::new(
            "YEAR_UNDECLARED",
            Status::Indeterminate,
            "Fiscal scope is absent; this may be a posting batch",
        ));
    }
    for (id, dim) in &d.dimensions {
        let mut seen = BTreeSet::new();
        let mut current = Some(id.as_str());
        while let Some(c) = current {
            if !seen.insert(c.to_owned()) {
                emit(Finding::new(
                    "DIMENSION_CYCLE",
                    Status::Fail,
                    "Dimension hierarchy contains a cycle",
                ));
                break;
            }
            match d.dimensions.get(c) {
                Some(v) => current = v.parent.as_deref(),
                None => {
                    emit(Finding::new(
                        "DIMENSION_PARENT",
                        Status::Indeterminate,
                        "A parent dimension is undeclared",
                    ));
                    break;
                }
            }
        }
        if id != &dim.id {
            emit(Finding::new(
                "DIMENSION_KEY",
                Status::Fail,
                "Dimension key differs from its identity",
            ));
        }
    }
    let mut object_keys = BTreeSet::new();
    for o in &d.objects {
        if !object_keys.insert((o.dimension.clone(), o.number.clone())) {
            emit(Finding::new(
                "OBJECT_DUPLICATE",
                Status::Fail,
                "Object declared more than once",
            ));
        }
        if o.number.is_empty() || !d.dimensions.contains_key(&o.dimension) {
            emit(Finding::new(
                "OBJECT_DECLARATION",
                Status::Indeterminate,
                "Object identity or dimension declaration is incomplete",
            ));
        }
    }
    let mut ids = BTreeSet::new();
    let mut totals = Totals::default();
    for v in &d.vouchers {
        totals.vouchers += 1;
        date(&v.date)?;
        if let Some(d) = &v.registered_on {
            date(d)?;
        }
        let period = d.year_for(&v.date);
        if period.is_none() {
            emit(
                Finding::new(
                    "VOUCHER_YEAR",
                    Status::Indeterminate,
                    "Voucher cannot be uniquely assigned to a fiscal interval",
                )
                .at(&v.at),
            );
        }
        if !v.series.is_empty() && !v.number.is_empty() {
            if let Some((_, p)) = period {
                if !ids.insert((p.clone(), v.series.clone(), v.number.clone())) {
                    emit(
                        Finding::new(
                            "VOUCHER_ID_DUPLICATE",
                            Status::Fail,
                            "Voucher identity repeats in the same fiscal interval",
                        )
                        .at(&v.at),
                    );
                }
            }
        }
        let mut sum = Money::ZERO;
        let mut finals = 0;
        for (i, r) in v.rows.iter().enumerate() {
            parse::numeric(&r.account)?;
            if let Some(d) = &r.date {
                date(d)?;
            }
            if !d.accounts.contains_key(&r.account) {
                emit(
                    Finding::new(
                        "ACCOUNT_UNDECLARED",
                        Status::Indeterminate,
                        "Transaction account has no declared chart entry",
                    )
                    .at(&r.at),
                );
            }
            check_assignments(&r.objects, d, &object_keys, &r.at, &mut emit);
            if let Some((_, p)) = period {
                if !p.contains(r.effective_date(v)) {
                    emit(Finding::new("CROSS_YEAR_ROW",Status::Indeterminate,"Cross-year voucher is outside this implementation's supported projection").at(&r.at));
                }
            }
            if r.kind == RowKind::Final {
                sum = sum.checked_add(r.amount)?;
                totals.final_rows += 1;
                finals += 1;
            } else {
                totals.history_rows += 1;
            }
            if r.kind == RowKind::AddedHistory {
                let paired = v.rows.get(i + 1).is_some_and(|next| {
                    next.kind == RowKind::Final
                        && same_row(r, next)
                        && next.at.record == r.at.record + 1
                });
                if !paired {
                    emit(
                        Finding::new(
                            "RTRANS_PAIR",
                            Status::Fail,
                            "Supplementary row lacks its immediately following identical TRANS row",
                        )
                        .at(&r.at),
                    );
                }
            }
        }
        if finals == 0 {
            emit(
                Finding::new(
                    "NO_FINAL_ROWS",
                    Status::Indeterminate,
                    "Voucher has no final transaction rows",
                )
                .at(&v.at),
            );
        }
        if sum != Money::ZERO {
            emit(
                Finding::new(
                    "VOUCHER_UNBALANCED",
                    Status::Fail,
                    "Final transaction amounts do not sum to zero",
                )
                .at(&v.at),
            );
        }
        totals.final_sum = totals.final_sum.checked_add(sum)?;
    }
    if d.sie_type != 4 && !d.vouchers.is_empty() {
        emit(Finding::new(
            "TYPE_HAS_TRANSACTIONS",
            Status::Fail,
            "Transactions occur in a balance-only file type",
        ));
    }
    let mut keys = BTreeSet::new();
    for b in &d.balances {
        let Some(p) = d.years.get(&b.year) else {
            emit(
                Finding::new(
                    "BALANCE_YEAR",
                    Status::Indeterminate,
                    "Balance references an undeclared fiscal year",
                )
                .at(&b.at),
            );
            continue;
        };
        if !keys.insert((
            b.kind,
            p.clone(),
            b.month.clone(),
            b.account.clone(),
            b.objects.clone(),
        )) {
            emit(
                Finding::new(
                    "BALANCE_DUPLICATE",
                    Status::Fail,
                    "Duplicate balance/control key",
                )
                .at(&b.at),
            );
        }
        if !d.accounts.contains_key(&b.account) {
            emit(
                Finding::new(
                    "BALANCE_ACCOUNT",
                    Status::Indeterminate,
                    "Balance account is undeclared",
                )
                .at(&b.at),
            );
        }
        check_assignments(&b.objects, d, &object_keys, &b.at, &mut emit);
        if let Some(m) = &b.month {
            month(m)?;
            if m.as_str() < &p.start[..6] || m.as_str() > &p.end[..6] {
                emit(
                    Finding::new(
                        "BALANCE_MONTH",
                        Status::Fail,
                        "Period movement month lies outside its fiscal year",
                    )
                    .at(&b.at),
                );
            }
        }
    }
    drop(emit);
    if cut && status == Status::Pass {
        status = Status::Indeterminate;
    }
    Ok(Validation {
        schema: "siediff.validation.v1".into(),
        status,
        findings: f,
        findings_truncated: cut,
        totals,
    })
}
fn check_assignments(
    a: &[Assignment],
    d: &Document,
    keys: &BTreeSet<(String, String)>,
    at: &Location,
    emit: &mut impl FnMut(Finding),
) {
    let mut seen = BTreeSet::new();
    for x in a {
        if !seen.insert(x.dimension.clone()) {
            emit(
                Finding::new(
                    "DIMENSION_REPEATED",
                    Status::Fail,
                    "Row repeats a dimension",
                )
                .at(at),
            );
        }
        if !d.dimensions.contains_key(&x.dimension)
            || !keys.contains(&(x.dimension.clone(), x.object.clone()))
        {
            emit(
                Finding::new(
                    "ASSIGNMENT_UNDECLARED",
                    Status::Indeterminate,
                    "Dimension or object has not been declared",
                )
                .at(at),
            );
        }
        if let Some(parent) = d
            .dimensions
            .get(&x.dimension)
            .and_then(|d| d.parent.as_deref())
        {
            if !a.iter().any(|x| x.dimension == parent) {
                emit(
                    Finding::new(
                        "PARENT_ASSIGNMENT",
                        Status::Fail,
                        "Sub-dimension row omits the required parent assignment",
                    )
                    .at(at),
                );
            }
        }
    }
}
/// No result can authorize a write. This helper provides only observed final sums.
pub fn account_movements(d: &Document, p: &Period) -> Result<BTreeMap<String, Money>> {
    let mut out = BTreeMap::new();
    for v in &d.vouchers {
        for r in &v.rows {
            if r.kind == RowKind::Final && p.contains(r.effective_date(v)) {
                let prev = out.get(&r.account).copied().unwrap_or(Money::ZERO);
                out.insert(r.account.clone(), prev.checked_add(r.amount)?);
            }
        }
    }
    Ok(out)
}
pub fn ensure_bounded(d: &Document, l: &Limits) -> Result<()> {
    if d.years.len() > 32 || d.dimensions.len() > 500 {
        return Err(Error::new(
            "LIMIT_METADATA",
            "Fiscal years or dimensions exceed supported bounds",
        ));
    }
    if d.vouchers.len() > l.vouchers
        || d.balances.len() > l.records
        || d.accounts.len() > l.records
        || d.dimensions.len() > l.records
        || d.objects.len() > l.records
    {
        return Err(Error::new(
            "LIMIT_SNAPSHOT",
            "Snapshot exceeds element limits",
        ));
    }
    let mut rows = 0usize;
    for v in &d.vouchers {
        if v.rows.len() > l.rows_per_voucher {
            return Err(Error::new("LIMIT_ROWS", "Too many rows"));
        }
        rows = rows
            .checked_add(v.rows.len())
            .ok_or_else(|| Error::new("LIMIT_ROWS", "Row count overflow"))?;
    }
    if rows > l.records {
        return Err(Error::new("LIMIT_ROWS", "Too many total rows"));
    }
    Ok(())
}
