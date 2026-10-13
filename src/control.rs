//! Checks explicit independent observations. The observations' provenance and
//! completeness remain caller-supplied premises, never conclusions of hashing.
use crate::{digest, model::*, money::Money, valid_hash, validate, Error, Result, SEMANTICS};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    CompleteAsserted,
    Partial,
    Unknown,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Opening,
    Closing,
    Movement,
    Bridge,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub through: Option<String>,
    pub id: String,
    pub kind: Kind,
    pub account: String,
    pub objects: Vec<Assignment>,
    pub expected: Money,
    pub evidence_sha256: String,
    pub evidence_locator: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub ledger_sha256: String,
    pub organization: String,
    pub organization_parts: Vec<String>,
    pub currency: String,
    pub fiscal_period: Period,
    pub through: String,
    pub coverage: Coverage,
    pub omission_means_zero: bool,
    pub review_ref: String,
    pub controls: Vec<Control>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: String,
    pub status: Status,
    pub expected: Money,
    pub observed: Option<Money>,
    pub delta: Option<Money>,
    pub ledger_locations: Vec<Location>,
    pub evidence_sha256: String,
    pub evidence_locator: String,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlReport {
    pub schema: String,
    pub semantics_version: String,
    pub ledger_sha256: String,
    pub manifest_sha256: String,
    pub status: Status,
    pub observations: Vec<Observation>,
    pub premises: Vec<String>,
    pub accountant_acceptance: Status,
}
fn matches_objects(row: &[Assignment], filter: &[Assignment]) -> bool {
    filter.iter().all(|f| row.contains(f))
}
fn declared(
    d: &Document,
    year: i32,
    kind: BalanceKind,
    account: &str,
    objects: &[Assignment],
) -> Option<(Money, Location)> {
    d.balances
        .iter()
        .find(|b| b.year == year && b.kind == kind && b.account == account && b.objects == objects)
        .map(|b| (b.amount, b.at.clone()))
}
pub fn check(d: &Document, m: &Manifest, l: &Limits) -> Result<ControlReport> {
    if m.schema != "siediff.controls.v1"
        || m.ledger_sha256 != d.sha256
        || m.review_ref.trim().is_empty()
    {
        return Err(Error::new(
            "CONTROL_BASIS",
            "Control manifest is not bound to this input and a review reference",
        ));
    }
    Period::new(&m.fiscal_period.start, &m.fiscal_period.end)?;
    date(&m.through)?;
    if !m.fiscal_period.contains(&m.through)
        || m.controls.is_empty()
        || m.controls.len() > l.findings
    {
        return Err(Error::new(
            "CONTROL_SCOPE",
            "Invalid control scope or control count",
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for c in &m.controls {
        if c.from.is_some() != c.through.is_some() || (c.from.is_some() && c.kind != Kind::Movement)
        {
            return Err(Error::new(
                "CONTROL_WINDOW",
                "Both date bounds are required and apply only to movements",
            ));
        }
        if let (Some(from), Some(through)) = (&c.from, &c.through) {
            let window = Period::new(from, through)?;
            if !m.fiscal_period.contains(&window.start)
                || !m.fiscal_period.contains(&window.end)
                || window.end > m.through
            {
                return Err(Error::new(
                    "CONTROL_WINDOW",
                    "Movement window exceeds the manifest scope",
                ));
            }
        }
        if c.kind == Kind::Bridge && c.expected != Money::ZERO {
            return Err(Error::new(
                "CONTROL_BRIDGE",
                "A bridge residual must be checked against zero",
            ));
        }
        if c.id.is_empty()
            || !ids.insert(c.id.clone())
            || !valid_hash(&c.evidence_sha256)
            || c.evidence_locator.is_empty()
        {
            return Err(Error::new(
                "CONTROL_IDENTITY",
                "Missing, repeated or invalid control evidence identity",
            ));
        }
        crate::parse::numeric(&c.account)?;
        let mut dims = std::collections::BTreeSet::new();
        for o in &c.objects {
            if !dims.insert(&o.dimension) {
                return Err(Error::new(
                    "CONTROL_DIMENSION",
                    "Repeated control dimension",
                ));
            }
        }
    }
    let validation = validate::validate(d, l)?;
    let entity = if d.organization.as_deref() == Some(m.organization.as_str())
        && d.organization_parts == m.organization_parts
        && d.currency == m.currency
    {
        Status::Pass
    } else {
        Status::Fail
    };
    let year = d
        .years
        .iter()
        .find(|(_, p)| *p == &m.fiscal_period)
        .map(|(y, _)| *y);
    let mut all = vec![];
    for c in &m.controls {
        let mut objects = c.objects.clone();
        objects.sort();
        let opening_kind = if objects.is_empty() {
            BalanceKind::Opening
        } else {
            BalanceKind::ObjectOpening
        };
        let closing_kind = if objects.is_empty() {
            BalanceKind::Closing
        } else {
            BalanceKind::ObjectClosing
        };
        let o = year.and_then(|y| declared(d, y, opening_kind, &c.account, &objects));
        let u = year.and_then(|y| declared(d, y, closing_kind, &c.account, &objects));
        let allowed_zero = m.coverage == Coverage::CompleteAsserted
            && m.omission_means_zero
            && d.accounts.contains_key(&c.account);
        let opening = o.as_ref().map(|x| x.0).or(if allowed_zero {
            Some(Money::ZERO)
        } else {
            None
        });
        let closing = u.as_ref().map(|x| x.0).or(if allowed_zero {
            Some(Money::ZERO)
        } else {
            None
        });
        let mut movement = Money::ZERO;
        let mut locations = vec![];
        if let Some((_, at)) = &o {
            locations.push(at.clone())
        }
        if let Some((_, at)) = &u {
            locations.push(at.clone())
        }
        for v in &d.vouchers {
            for r in &v.rows {
                let day = r.effective_date(v);
                if r.kind == RowKind::Final
                    && r.account == c.account
                    && matches_objects(&r.objects, &objects)
                    && day >= c.from.as_deref().unwrap_or(m.fiscal_period.start.as_str())
                    && day <= c.through.as_deref().unwrap_or(m.through.as_str())
                {
                    movement = movement.checked_add(r.amount)?;
                    if locations.len() < 50 {
                        locations.push(r.at.clone())
                    }
                }
            }
        }
        let through_ok = d
            .extent_date
            .as_deref()
            .map_or(m.through == m.fiscal_period.end, |s| s == m.through);
        let observed = match c.kind {
            Kind::Opening => opening,
            Kind::Closing => {
                if through_ok {
                    closing
                } else {
                    None
                }
            }
            Kind::Movement => {
                if d.sie_type == 4 {
                    Some(movement)
                } else {
                    None
                }
            }
            Kind::Bridge => {
                if through_ok && d.sie_type == 4 {
                    match (opening, closing) {
                        (Some(o), Some(u)) => Some(o.checked_add(movement)?.checked_sub(u)?),
                        _ => None,
                    }
                } else {
                    None
                }
            }
        };
        let mut s = match observed {
            Some(x) if x == c.expected => Status::Pass,
            Some(_) => Status::Fail,
            None => Status::Indeterminate,
        };
        s = combine([s, validation.status, entity]);
        if year.is_none() || m.coverage != Coverage::CompleteAsserted {
            s = combine([s, Status::Indeterminate]);
        }
        all.push(Observation {
            id: c.id.clone(),
            status: s,
            expected: c.expected,
            observed,
            delta: observed.map(|x| x.checked_sub(c.expected)).transpose()?,
            ledger_locations: locations,
            evidence_sha256: c.evidence_sha256.clone(),
            evidence_locator: c.evidence_locator.clone(),
            reason: if observed.is_none() {
                "Required balance or its date basis is not established"
            } else if m.coverage != Coverage::CompleteAsserted {
                "Observation is conditional on incomplete or unknown source coverage"
            } else {
                "Exact comparison under the supplied scope and evidence assertions"
            }
            .into(),
        });
    }
    Ok(ControlReport{schema:"siediff.control-report.v1".into(),semantics_version:SEMANTICS.into(),ledger_sha256:d.sha256.clone(),manifest_sha256:digest(m)?,status:combine(all.iter().map(|x|x.status)),observations:all,premises:vec![
        "Independent controls were supplied by a trusted operator; the engine does not authenticate their author or financial truth.".into(),
        format!("Coverage premise: {:?}; omission-means-zero premise: {}",m.coverage,m.omission_means_zero),
        "Document support, treatment correctness and accountant acceptance remain separate.".into(),
        "Location lists are bounded; they are not complete transaction inventories.".into(),
    ],accountant_acceptance:Status::Indeterminate})
}
