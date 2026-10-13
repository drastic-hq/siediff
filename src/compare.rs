use crate::{
    digest,
    model::*,
    money::Money,
    validate::{self, Validation},
    Error, Result, SEMANTICS, VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Identity,
    Renumbered,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub mode: Mode,
    pub period: Option<Period>,
    pub mapping: Option<Mapping>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectMapping {
    pub dimension: String,
    pub object: String,
    pub target_object: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    pub schema: String,
    pub left_sha256: String,
    pub right_sha256: String,
    pub review_ref: String,
    pub accounts: BTreeMap<String, String>,
    pub dimensions: BTreeMap<String, String>,
    pub objects: BTreeMap<String, BTreeMap<String, String>>,
}
impl Mapping {
    fn account(&self, s: &str) -> String {
        self.accounts.get(s).cloned().unwrap_or_else(|| s.into())
    }
    fn dimension(&self, s: &str) -> String {
        self.dimensions.get(s).cloned().unwrap_or_else(|| s.into())
    }
    fn object(&self, d: &str, o: &str) -> String {
        self.objects
            .get(d)
            .and_then(|m| m.get(o))
            .cloned()
            .unwrap_or_else(|| o.into())
    }
    fn check(&self, a: &Document, b: &Document) -> Result<()> {
        if self.schema != "siediff.mapping.v1"
            || self.left_sha256 != a.sha256
            || self.right_sha256 != b.sha256
            || self.review_ref.trim().is_empty()
        {
            return Err(Error::new(
                "MAPPING_BASIS",
                "Mapping is not bound to these exact inputs and a review reference",
            ));
        }
        for (s, t) in &self.accounts {
            if !a.accounts.contains_key(s) || !b.accounts.contains_key(t) {
                return Err(Error::new(
                    "MAPPING_ACCOUNT",
                    "Account mapping references undeclared account",
                ));
            }
            let x = a.accounts[s].kind.as_deref();
            let y = b.accounts[t].kind.as_deref();
            if x.is_some() && y.is_some() && x != y {
                return Err(Error::new(
                    "MAPPING_CLASS",
                    "Account mapping changes declared accounting class",
                ));
            }
        }
        let mut seen = BTreeSet::new();
        for s in a.accounts.keys() {
            if !seen.insert(self.account(s)) {
                return Err(Error::new(
                    "MAPPING_COLLISION",
                    "Effective account mapping is not injective",
                ));
            }
        }
        for (s, t) in &self.dimensions {
            if !a.dimensions.contains_key(s) || !b.dimensions.contains_key(t) {
                return Err(Error::new(
                    "MAPPING_DIMENSION",
                    "Dimension mapping references undeclared dimension",
                ));
            }
        }
        seen.clear();
        for s in a.dimensions.keys() {
            if !seen.insert(self.dimension(s)) {
                return Err(Error::new(
                    "MAPPING_COLLISION",
                    "Effective dimension mapping is not injective",
                ));
            }
        }
        let source_objects: BTreeSet<_> = a
            .objects
            .iter()
            .map(|o| (o.dimension.as_str(), o.number.as_str()))
            .collect();
        let target_objects: BTreeSet<_> = b
            .objects
            .iter()
            .map(|o| (o.dimension.clone(), o.number.clone()))
            .collect();
        let mut keys = BTreeSet::new();
        for (dim, entries) in &self.objects {
            for (object, target) in entries {
                if !source_objects.contains(&(dim.as_str(), object.as_str()))
                    || !target_objects.contains(&(self.dimension(dim), target.clone()))
                {
                    return Err(Error::new(
                        "MAPPING_OBJECT",
                        "Object mapping references undeclared values",
                    ));
                }
            }
        }
        keys.clear();
        for o in &a.objects {
            if !keys.insert((
                self.dimension(&o.dimension),
                self.object(&o.dimension, &o.number),
            )) {
                return Err(Error::new(
                    "MAPPING_COLLISION",
                    "Effective object mapping is not injective",
                ));
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assertion {
    pub id: String,
    pub status: Status,
    pub statement: String,
    pub premises: Vec<String>,
    pub rule_version: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchGroup {
    pub financial_fingerprint: String,
    pub left: Vec<Location>,
    pub right: Vec<Location>,
    pub unique_correspondence: bool,
    pub left_count: usize,
    pub right_count: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountDelta {
    pub account: String,
    pub left: Money,
    pub right: Money,
    pub right_minus_left: Money,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Difference {
    pub code: String,
    pub left: Vec<Location>,
    pub right: Vec<Location>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub explanations: Vec<crate::explanations::Explanation>,
    pub explanations_truncated: bool,
    pub required_work: Vec<crate::work::RequiredWork>,
    pub work_basis_sha256: Option<String>,
    pub schema: String,
    pub engine_version: String,
    pub semantics_version: String,
    pub report_id: String,
    pub left_sha256: String,
    pub right_sha256: String,
    pub policy_sha256: String,
    pub policy: Policy,
    pub left_validation: Validation,
    pub right_validation: Validation,
    pub assertions: Vec<Assertion>,
    pub groups: Vec<MatchGroup>,
    pub differences: Vec<Difference>,
    pub differences_truncated: bool,
    pub account_deltas: Vec<AccountDelta>,
    pub limitations: Vec<String>,
}
impl Report {
    pub fn status(&self, id: &str) -> Status {
        self.assertions
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.status)
            .unwrap_or(Status::Indeterminate)
    }
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct FinRow {
    account: String,
    objects: Vec<Assignment>,
    amount: Money,
    date: String,
    quantity: Option<String>,
    unit: Option<String>,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct FinVoucher {
    period: Option<Period>,
    date: String,
    rows: Vec<FinRow>,
}
fn mapped_objects(a: &[Assignment], m: Option<&Mapping>) -> Vec<Assignment> {
    let mut out: Vec<_> = a
        .iter()
        .map(|o| Assignment {
            dimension: m
                .map(|m| m.dimension(&o.dimension))
                .unwrap_or_else(|| o.dimension.clone()),
            object: m
                .map(|m| m.object(&o.dimension, &o.object))
                .unwrap_or_else(|| o.object.clone()),
        })
        .collect();
    out.sort();
    out
}
fn account(s: &str, m: Option<&Mapping>) -> String {
    m.map(|m| m.account(s)).unwrap_or_else(|| s.into())
}
fn selected<'a>(d: &'a Document, p: &Policy) -> Vec<&'a Voucher> {
    d.vouchers
        .iter()
        .filter(|v| p.period.as_ref().is_none_or(|p| p.contains(&v.date)))
        .collect()
}
fn financial(d: &Document, v: &Voucher, m: Option<&Mapping>) -> FinVoucher {
    let mut rows: Vec<_> = v
        .rows
        .iter()
        .filter(|r| r.kind == RowKind::Final)
        .map(|r| FinRow {
            account: account(&r.account, m),
            objects: mapped_objects(&r.objects, m),
            amount: r.amount,
            date: r.effective_date(v).into(),
            quantity: r.quantity.clone(),
            unit: if r.quantity.is_some() {
                d.accounts.get(&r.account).and_then(|a| a.unit.clone())
            } else {
                None
            },
        })
        .collect();
    rows.sort();
    FinVoucher {
        period: d.year_for(&v.date).map(|(_, p)| p.clone()),
        date: v.date.clone(),
        rows,
    }
}
fn row_groups(
    d: &Document,
    v: &Voucher,
    m: Option<&Mapping>,
) -> Result<BTreeMap<String, Vec<Location>>> {
    let mut out: BTreeMap<String, Vec<Location>> = BTreeMap::new();
    for r in &v.rows {
        if r.kind == RowKind::Final {
            let k = key(&(
                account(&r.account, m),
                mapped_objects(&r.objects, m),
                r.amount,
                r.effective_date(v),
                &r.quantity,
                if r.quantity.is_some() {
                    d.accounts.get(&r.account).and_then(|a| a.unit.as_deref())
                } else {
                    None
                },
            ))?;
            out.entry(k).or_default().push(r.at.clone());
        }
    }
    Ok(out)
}
fn identity(d: &Document, v: &Voucher) -> Option<String> {
    if v.series.is_empty() || v.number.is_empty() {
        return None;
    }
    let (_, p) = d.year_for(&v.date)?;
    serde_json::to_string(&(p, &v.series, &v.number)).ok()
}
fn key<T: Serialize>(v: &T) -> Result<String> {
    serde_json::to_string(v)
        .map_err(|_| Error::new("SERIALIZE", "Cannot form canonical comparison key"))
}
fn groups<'a>(
    d: &Document,
    v: &[&'a Voucher],
    m: Option<&Mapping>,
) -> Result<BTreeMap<String, Vec<&'a Voucher>>> {
    let mut out: BTreeMap<String, Vec<&Voucher>> = BTreeMap::new();
    for v in v {
        out.entry(key(&financial(d, v, m))?).or_default().push(*v);
    }
    Ok(out)
}
fn history(v: &Voucher, m: Option<&Mapping>) -> Result<String> {
    // Preserve order and explicit dates. Not a reconstruction of omitted history.
    let rows: Vec<_> = v
        .rows
        .iter()
        .map(|r| {
            (
                r.kind,
                account(&r.account, m),
                mapped_objects(&r.objects, m),
                r.amount,
                &r.date,
                &r.text,
                &r.quantity,
                &r.signature,
            )
        })
        .collect();
    key(&(&v.date, &v.text, &v.registered_on, &v.signature, rows))
}
fn assertion(id: &str, s: Status, statement: &str, premises: &[&str]) -> Assertion {
    Assertion {
        id: id.into(),
        status: s,
        statement: statement.into(),
        premises: premises.iter().map(|x| (*x).into()).collect(),
        rule_version: SEMANTICS.into(),
    }
}
fn same<T: PartialEq>(a: &T, b: &T) -> Status {
    if a == b {
        Status::Pass
    } else {
        Status::Fail
    }
}
fn balances(d: &Document, p: &Policy, m: Option<&Mapping>) -> Result<Vec<String>> {
    let mut out = vec![];
    for b in &d.balances {
        let period = d.years.get(&b.year);
        let include = match &p.period {
            None => true,
            Some(s) => {
                if let Some(mon) = &b.month {
                    mon.as_str() >= &s.start[..6] && mon.as_str() <= &s.end[..6]
                } else {
                    period == Some(s)
                }
            }
        };
        if include {
            out.push(key(&(
                b.kind,
                period,
                &b.month,
                account(&b.account, m),
                mapped_objects(&b.objects, m),
                b.amount,
                &b.quantity,
            ))?);
        }
    }
    out.sort();
    Ok(out)
}
fn chart(d: &Document, m: Option<&Mapping>) -> Result<String> {
    let mut a: Vec<_> = d
        .accounts
        .values()
        .map(|a| (account(&a.number, m), &a.name, &a.kind, &a.unit))
        .collect();
    a.sort();
    let mut dims: Vec<_> = d
        .dimensions
        .values()
        .map(|d| {
            (
                m.map(|m| m.dimension(&d.id))
                    .unwrap_or_else(|| d.id.clone()),
                &d.name,
                d.parent
                    .as_ref()
                    .map(|s| m.map(|m| m.dimension(s)).unwrap_or_else(|| s.clone())),
            )
        })
        .collect();
    dims.sort();
    let mut objects: Vec<_> = d
        .objects
        .iter()
        .map(|o| {
            (
                m.map(|m| m.dimension(&o.dimension))
                    .unwrap_or_else(|| o.dimension.clone()),
                m.map(|m| m.object(&o.dimension, &o.number))
                    .unwrap_or_else(|| o.number.clone()),
                &o.name,
            )
        })
        .collect();
    objects.sort();
    key(&(a, dims, objects))
}
pub fn compare(a: &Document, b: &Document, p: &Policy, l: &Limits) -> Result<Report> {
    if let Some(scope) = &p.period {
        Period::new(&scope.start, &scope.end)?;
    }
    if let Some(m) = &p.mapping {
        m.check(a, b)?;
    }
    let av = validate::validate(a, l)?;
    let bv = validate::validate(b, l)?;
    let valid = combine([av.status, bv.status]);
    let entity = match (&a.organization, &b.organization) {
        (Some(x), Some(y)) if !x.is_empty() && !y.is_empty() => {
            same(&(x, &a.organization_parts), &(y, &b.organization_parts))
        }
        _ => Status::Indeterminate,
    };
    let scope = combine([
        entity,
        same(&a.currency, &b.currency),
        if p.mapping.is_some() {
            Status::Pass
        } else {
            same(&a.chart_family, &b.chart_family)
        },
    ]);
    let aa = selected(a, p);
    let bb = selected(b, p);
    let ga = groups(a, &aa, p.mapping.as_ref())?;
    let gb = groups(b, &bb, None)?;
    let keys: BTreeSet<_> = ga.keys().chain(gb.keys()).cloned().collect();
    let mut match_groups = vec![];
    let mut differences = vec![];
    let mut truncated = false;
    let mut equal = true;
    let mut h_equal = true;
    let h_available = a
        .snapshot
        .as_ref()
        .is_none_or(|s| s.projection == "posted_with_sie_history_v1")
        && b.snapshot
            .as_ref()
            .is_none_or(|s| s.projection == "posted_with_sie_history_v1");
    for k in keys {
        let la = ga.get(&k).cloned().unwrap_or_default();
        let rb = gb.get(&k).cloned().unwrap_or_default();
        if h_available {
            let mut ha = la
                .iter()
                .map(|v| history(v, p.mapping.as_ref()))
                .collect::<Result<Vec<_>>>()?;
            let mut hb = rb
                .iter()
                .map(|v| history(v, None))
                .collect::<Result<Vec<_>>>()?;
            ha.sort();
            hb.sort();
            h_equal &= ha == hb;
        }
        if la.len() > 50 || rb.len() > 50 {
            truncated = true;
        }
        let loc_a: Vec<_> = la.iter().map(|v| v.at.clone()).collect();
        let loc_b: Vec<_> = rb.iter().map(|v| v.at.clone()).collect();
        if la.len() != rb.len() {
            equal = false;
            if differences.len() < l.findings {
                differences.push(Difference {
                    code: "VOUCHER_MULTISET_DIFFERENCE".into(),
                    left: loc_a.clone(),
                    right: loc_b.clone(),
                });
            } else {
                truncated = true;
            }
        }
        // Bounded output, but comparison always processes every group.
        if match_groups.len() < l.findings {
            match_groups.push(MatchGroup {
                financial_fingerprint: crate::sha256(k.as_bytes()),
                left: loc_a.into_iter().take(50).collect(),
                right: loc_b.into_iter().take(50).collect(),
                unique_correspondence: la.len() == 1 && rb.len() == 1,
                left_count: la.len(),
                right_count: rb.len(),
            });
        } else {
            truncated = true;
        }
    }
    let identity_status = {
        let mut ia = BTreeMap::new();
        let mut ib = BTreeMap::new();
        let mut ids_complete = true;
        for v in &aa {
            if let Some(id) = identity(a, v) {
                ia.insert(id, *v);
            } else {
                ids_complete = false
            }
        }
        for v in &bb {
            if let Some(id) = identity(b, v) {
                ib.insert(id, *v);
            } else {
                ids_complete = false
            }
        }
        if valid == Status::Pass {
            for (id, v) in &ia {
                if let Some(w) = ib.get(id) {
                    if financial(a, v, p.mapping.as_ref()) != financial(b, w, None) {
                        if differences.len() < l.findings {
                            differences.push(Difference {
                                code: "SAME_ID_POSTINGS_CHANGED".into(),
                                left: vec![v.at.clone()],
                                right: vec![w.at.clone()],
                            });
                        } else {
                            truncated = true;
                        }
                        let ra = row_groups(a, v, p.mapping.as_ref())?;
                        let rb = row_groups(b, w, None)?;
                        for k in ra.keys().chain(rb.keys()).collect::<BTreeSet<_>>() {
                            let x = ra.get(k).cloned().unwrap_or_default();
                            let y = rb.get(k).cloned().unwrap_or_default();
                            if x.len() != y.len() {
                                if x.len() > 50 || y.len() > 50 {
                                    truncated = true;
                                }
                                if differences.len() < l.findings {
                                    differences.push(Difference {
                                        code: "ROW_MULTISET_DIFFERENCE".into(),
                                        left: x.into_iter().take(50).collect(),
                                        right: y.into_iter().take(50).collect(),
                                    });
                                } else {
                                    truncated = true;
                                }
                            }
                        }
                    }
                }
            }
        }
        if aa.is_empty() && bb.is_empty() {
            Status::NotApplicable
        } else if ids_complete {
            if ia.keys().eq(ib.keys())
                && ia.iter().all(|(id, v)| {
                    financial(a, v, p.mapping.as_ref()) == financial(b, ib[id], None)
                })
            {
                Status::Pass
            } else {
                Status::Fail
            }
        } else {
            Status::Indeterminate
        }
    };
    let mut transaction_status = if aa.is_empty() && bb.is_empty() {
        Status::NotApplicable
    } else if equal {
        Status::Pass
    } else {
        Status::Fail
    };
    if p.mode == Mode::Identity && transaction_status != Status::NotApplicable {
        transaction_status = combine([transaction_status, identity_status]);
    }
    if let Some(period) = &p.period {
        let crosses = aa.iter().chain(bb.iter()).any(|v| {
            v.rows
                .iter()
                .any(|r| r.kind == RowKind::Final && !period.contains(r.effective_date(v)))
        });
        if crosses && transaction_status != Status::NotApplicable {
            transaction_status = combine([transaction_status, Status::Indeterminate]);
        }
    }
    if a.sie_type != 4 || b.sie_type != 4 {
        transaction_status = Status::NotApplicable;
    }
    if transaction_status != Status::NotApplicable {
        transaction_status = combine([transaction_status, scope, valid]);
    }
    let ba = balances(a, p, p.mapping.as_ref())?;
    let bc = balances(b, p, None)?;
    let balance_status = if ba.is_empty() && bc.is_empty() {
        Status::NotApplicable
    } else {
        combine([same(&ba, &bc), scope, valid])
    };
    let hs = if !h_available {
        Status::Indeterminate
    } else if aa.is_empty() && bb.is_empty() {
        Status::NotApplicable
    } else {
        combine([
            if h_equal { Status::Pass } else { Status::Fail },
            scope,
            valid,
        ])
    };
    let mut ma: BTreeMap<String, Money> = BTreeMap::new();
    let mut mb: BTreeMap<String, Money> = BTreeMap::new();
    for v in &aa {
        for r in &v.rows {
            if r.kind == RowKind::Final {
                let k = account(&r.account, p.mapping.as_ref());
                let old = ma.get(&k).copied().unwrap_or(Money::ZERO);
                ma.insert(k, old.checked_add(r.amount)?);
            }
        }
    }
    for v in &bb {
        for r in &v.rows {
            if r.kind == RowKind::Final {
                let old = mb.get(&r.account).copied().unwrap_or(Money::ZERO);
                mb.insert(r.account.clone(), old.checked_add(r.amount)?);
            }
        }
    }
    let accounts: BTreeSet<_> = ma.keys().chain(mb.keys()).cloned().collect();
    let mut deltas = vec![];
    let mut net_equal = true;
    for account in accounts {
        let x = ma.get(&account).copied().unwrap_or(Money::ZERO);
        let y = mb.get(&account).copied().unwrap_or(Money::ZERO);
        if x != y {
            net_equal = false;
            if deltas.len() < l.findings {
                deltas.push(AccountDelta {
                    account,
                    left: x,
                    right: y,
                    right_minus_left: y.checked_sub(x)?,
                });
            } else {
                truncated = true;
            }
        }
    }
    let mut metadata_a: Vec<_> = a
        .records
        .iter()
        .filter(|r| {
            matches!(
                r.tag.as_str(),
                "#PROSA" | "#FTYP" | "#KPTYP" | "#SRU" | "#TAXAR" | "#BKOD" | "#ADRESS"
            )
        })
        .map(|r| (&r.tag, &r.fields))
        .collect();
    metadata_a.sort();
    let mut metadata_b: Vec<_> = b
        .records
        .iter()
        .filter(|r| {
            matches!(
                r.tag.as_str(),
                "#PROSA" | "#FTYP" | "#KPTYP" | "#SRU" | "#TAXAR" | "#BKOD" | "#ADRESS"
            )
        })
        .map(|r| (&r.tag, &r.fields))
        .collect();
    metadata_b.sort();
    let assertions = vec![
        assertion(
            "native_relationships",
            crate::relationships::status(a, b),
            "Supplied native correction links agree",
            &[
                "Separate from retained SIE row history",
                "Coverage is an explicit caller assertion",
            ],
        ),
        assertion(
            "supported_profile",
            valid,
            "Supported parsing and internal checks",
            &["Not full SIE certification"],
        ),
        assertion(
            "entity_currency",
            scope,
            "Declared entity, reporting currency and chart family are compatible",
            &["Identifiers are assertions, not verified identity"],
        ),
        assertion(
            "supplied_postings",
            transaction_status,
            "Supplied final postings agree under the stated policy",
            &[
                "TRANS-only financial projection",
                "Voucher grouping, dates, dimensions and multiplicity retained",
                "No unseen transactions inferred",
            ],
        ),
        assertion(
            "voucher_identity",
            combine([identity_status, valid, scope]),
            "Scoped voucher identities and their effects agree",
            &["Identity includes actual fiscal interval"],
        ),
        assertion(
            "unique_correspondence",
            if aa.is_empty() && bb.is_empty() {
                Status::NotApplicable
            } else if !equal {
                Status::Fail
            } else if !truncated && match_groups.iter().all(|g| g.unique_correspondence) {
                combine([valid, scope])
            } else {
                Status::Indeterminate
            },
            "Financial groups establish a unique correspondence",
            &["Repeated identical vouchers are not silently paired"],
        ),
        assertion(
            "retained_history",
            hs,
            "Supplied row history and descriptive content agree",
            &[
                "Only history retained in the sources is compared",
                "Hashes do not authenticate actor signatures",
            ],
        ),
        assertion(
            "declared_balances",
            balance_status,
            "Explicitly supplied control records agree",
            &[
                "Missing is not implicitly zero",
                "PSALDO is compared as a period movement",
                "Budget is retained but never added to actual movements",
            ],
        ),
        assertion(
            "net_account_movements",
            if aa.is_empty() && bb.is_empty() {
                Status::NotApplicable
            } else {
                combine([
                    if net_equal {
                        Status::Pass
                    } else {
                        Status::Fail
                    },
                    valid,
                    scope,
                ])
            },
            "Net movements agree on the selected supplied vouchers",
            &["Weaker than voucher or dimension equivalence"],
        ),
        assertion(
            "chart_and_dimensions",
            same(&chart(a, p.mapping.as_ref())?, &chart(b, None)?),
            "Declared chart, dimension and object metadata agree",
            &["No account-name synonym inference"],
        ),
        assertion(
            "other_metadata",
            if a.snapshot.is_some() || b.snapshot.is_some() {
                Status::Indeterminate
            } else {
                same(&metadata_a, &metadata_b)
            },
            "Selected retained nonvolatile metadata agrees",
            &["Not a complete byte-fidelity claim"],
        ),
        assertion(
            "migration_readiness",
            Status::Indeterminate,
            "Migration readiness requires additional evidence and approval",
            &[
                "Source coverage",
                "Independent control checks",
                "Projection fidelity",
                "Authorized accountant acceptance",
            ],
        ),
        assertion(
            "accountant_acceptance",
            Status::Indeterminate,
            "Accountant acceptance is outside this engine's authority",
            &["No technical pass grants posting or cutover authority"],
        ),
    ];
    let policy_hash = digest(p)?;
    let report_id = digest(&(&a.sha256, &b.sha256, &policy_hash, VERSION, SEMANTICS))?;
    let (explanations, explanations_truncated) = crate::explanations::explain(a, b, p, l)?;
    Ok(Report{explanations, explanations_truncated, required_work: vec![], work_basis_sha256: None, schema:"siediff.report.v1".into(),engine_version:VERSION.into(),semantics_version:SEMANTICS.into(),report_id,left_sha256:a.sha256.clone(),right_sha256:b.sha256.clone(),policy_sha256:policy_hash,policy:p.clone(),left_validation:av,right_validation:bv,assertions,groups:match_groups,differences,differences_truncated:truncated,account_deltas:deltas,limitations:vec![
        "Read-only comparison, not certification, audit opinion or accounting treatment validation.".into(),
        "SIE type 4 does not itself distinguish a complete export from a posting batch.".into(),
        "Missing evidence and undisclosed accounts cannot be discovered from matching exports alone.".into(),
        "Financial fingerprints identify value groups, not unique source-document relationships.".into(),
        "Reports and hashes may be sensitive; no automatic public-release classification.".into(),
    ]})
}
