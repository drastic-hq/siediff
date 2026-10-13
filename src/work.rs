use crate::{compare::Report, control::Coverage, digest, model::*, valid_hash, Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Basis {
    pub schema: String,
    pub left_sha256: String,
    pub right_sha256: String,
    pub book_id: String,
    pub before_revision: String,
    pub after_revision: String,
    pub decisions: Vec<Decision>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Prepared,
    Approved,
    Posted,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub id: String,
    pub state: State,
    pub assertions: Vec<String>,
    pub inventories: Vec<Inventory>,
    pub requirements: Vec<Requirement>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub id: String,
    pub before: String,
    pub after: String,
    pub coverage: Coverage,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementState {
    Satisfied,
    Missing,
    Unknown,
    Refuted,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    RequestEvidence,
    ReviewTreatment,
    ReviewCorrection,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub id: String,
    pub state: RequirementState,
    pub route: Route,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredWork {
    pub decision_id: String,
    pub action: String,
    pub lost_support: Vec<String>,
    pub unresolved_requirements: Vec<String>,
    pub requirements: Vec<Requirement>,
    pub authorizes_execution: bool,
}
pub fn apply(
    report: &mut Report,
    a: &Document,
    b: &Document,
    basis: Basis,
    l: &Limits,
) -> Result<()> {
    if basis.schema != "siediff.work-basis.v1"
        || basis.left_sha256 != a.sha256
        || basis.right_sha256 != b.sha256
        || basis.book_id.is_empty()
        || !a
            .snapshot
            .as_ref()
            .is_some_and(|s| s.book_id == basis.book_id && s.revision == basis.before_revision)
        || !b
            .snapshot
            .as_ref()
            .is_some_and(|s| s.book_id == basis.book_id && s.revision == basis.after_revision)
    {
        return Err(Error::new(
            "WORK_BASIS",
            "Work basis must bind both snapshots and one book's revisions",
        ));
    }
    if basis.decisions.len() > l.findings {
        return Err(Error::new("LIMIT_WORK", "Too many decisions"));
    }
    let basis_hash = digest(&basis)?;
    let mut ids = BTreeSet::new();
    for decision in basis.decisions {
        if decision.id.is_empty()
            || !ids.insert(decision.id.clone())
            || decision.assertions.is_empty()
            || decision.assertions.len() + decision.inventories.len() + decision.requirements.len()
                > l.findings
        {
            return Err(Error::new(
                "WORK_DECISION",
                "Missing, repeated or excessive dependency declarations",
            ));
        }
        let mut deps = BTreeSet::new();
        let mut lost_support = vec![];
        for id in &decision.assertions {
            if !deps.insert(format!("assertion:{id}"))
                || !report.assertions.iter().any(|a| &a.id == id)
            {
                return Err(Error::new(
                    "WORK_ASSERTION",
                    "Unknown or repeated assertion dependency",
                ));
            }
            if report.status(id) != Status::Pass {
                lost_support.push(format!("assertion:{id}"));
            }
        }
        for inventory in decision.inventories {
            if inventory.id.is_empty()
                || !valid_hash(&inventory.before)
                || !valid_hash(&inventory.after)
                || !deps.insert(format!("inventory:{}", inventory.id))
            {
                return Err(Error::new("WORK_INVENTORY", "Invalid inventory dependency"));
            }
            if inventory.before != inventory.after
                || inventory.coverage != Coverage::CompleteAsserted
            {
                lost_support.push(format!("inventory:{}", inventory.id));
            }
        }
        let mut unresolved = vec![];
        for requirement in &decision.requirements {
            if requirement.id.is_empty() || !deps.insert(format!("requirement:{}", requirement.id))
            {
                return Err(Error::new(
                    "WORK_REQUIREMENT",
                    "Invalid requirement identity",
                ));
            }
            if !matches!(requirement.state, RequirementState::Satisfied) {
                unresolved.push(requirement.id.clone());
            }
        }
        let changed = !lost_support.is_empty() || !unresolved.is_empty();
        let action = match (&decision.state, changed, unresolved.is_empty()) {
            (_, false, _) => "retain_support",
            (State::Posted, true, _) => "review_correction",
            (State::Approved, true, _) => "requalify_approval",
            (State::Prepared, true, false) => "resolve_requirements",
            _ => "requalify_preparation",
        };
        report.required_work.push(RequiredWork {
            decision_id: decision.id,
            action: action.into(),
            lost_support,
            unresolved_requirements: unresolved,
            requirements: decision.requirements,
            authorizes_execution: false,
        });
    }
    report.report_id = digest(&(&report.report_id, &basis_hash))?;
    report.work_basis_sha256 = Some(basis_hash);
    Ok(())
}
