use siediff::{
    self,
    compare::{self, Mapping, Mode, Policy, Report},
    control,
    model::*,
    money::{quantity, Money},
    parse, snapshot, strict_json, validate,
};
use std::{collections::BTreeMap, fs, path::Path};
fn bytes(name: &str) -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(name),
    )
    .unwrap()
}
fn doc(name: &str) -> Document {
    parse::parse(&bytes(name), Encoding::Cp437, &Limits::default()).unwrap()
}
fn diff(name: &str, mode: Mode) -> Report {
    compare::compare(
        &doc("base.se"),
        &doc(name),
        &Policy {
            mode,
            ..Policy::default()
        },
        &Limits::default(),
    )
    .unwrap()
}
#[test]
fn exact_amount() {
    assert_eq!(Money::major("12.50").unwrap().0, 1250);
}
#[test]
fn whole_amount() {
    assert_eq!(Money::major("12").unwrap().0, 1200);
}
#[test]
fn one_decimal() {
    assert_eq!(Money::major("12.5").unwrap().0, 1250);
}
#[test]
fn negative_amount() {
    assert_eq!(Money::major("-12.50").unwrap().0, -1250);
}
#[test]
fn negative_zero_canonical() {
    assert_eq!(Money::major("-0.00").unwrap().decimal(), "0.00");
}
#[test]
fn money_max_roundtrip() {
    assert_eq!(
        Money::major(&Money(i128::MAX).decimal()).unwrap(),
        Money(i128::MAX)
    );
}
#[test]
fn money_min_roundtrip() {
    assert_eq!(
        Money::major(&Money(i128::MIN).decimal()).unwrap(),
        Money(i128::MIN)
    );
}
#[test]
fn overflow_add() {
    assert!(Money(i128::MAX).checked_add(Money(1)).is_err());
}
#[test]
fn overflow_sub() {
    assert!(Money(i128::MIN).checked_sub(Money(1)).is_err());
}
#[test]
fn money_json_string() {
    assert_eq!(serde_json::to_string(&Money(42)).unwrap(), "\"42\"");
}
#[test]
fn reject_json_numeric_money() {
    assert!(serde_json::from_str::<Money>("42").is_err());
}
#[test]
fn invalid_amounts() {
    for x in [
        "", "+1", "1,00", "1e2", "NaN", "1.001", "1.", "--1", " 1", "1_000",
    ] {
        assert!(Money::major(x).is_err(), "{x}");
    }
}
#[test]
fn quantity_not_money() {
    assert_eq!(quantity("0002.5000").unwrap(), Some("2.5".into()));
}
#[test]
fn quantity_unknown_is_not_zero() {
    assert_ne!(quantity("").unwrap(), quantity("0").unwrap());
}
#[test]
fn quantity_limit() {
    assert!(quantity("1.1234567890123").is_err());
}
#[test]
fn amount_property_roundtrip() {
    let mut n = 1930u64;
    for _ in 0..20000 {
        n = n.wrapping_mul(6364136223846793005).wrapping_add(1);
        let x = Money((n as i64 as i128) * 1234567);
        assert_eq!(Money::major(&x.decimal()).unwrap(), x);
    }
}
#[test]
fn leap_year_2000() {
    assert!(date("20000229").is_ok());
}
#[test]
fn nonleap_1900() {
    assert!(date("19000229").is_err());
}
#[test]
fn invalid_month() {
    assert!(date("20260010").is_err());
    assert!(date("20261301").is_err());
}
#[test]
fn no_year_zero() {
    assert!(date("00000101").is_err());
}
#[test]
fn period_order() {
    assert!(Period::new("20260202", "20260201").is_err());
}
#[test]
fn cp437_roundtrip() {
    let s = "åäö ÅÄÖ é £";
    let b = siediff::encoding::cp437_encode(s).unwrap();
    assert_eq!(siediff::encoding::decode(&b, Encoding::Cp437).unwrap(), s);
}
#[test]
fn cp437_unrepresentable() {
    assert!(siediff::encoding::cp437_encode("💰").is_err());
}
#[test]
fn utf8_invalid_rejected() {
    assert!(siediff::encoding::decode(&[255], Encoding::Utf8).is_err());
}
#[test]
fn windows1252_undefined_rejected() {
    assert!(siediff::encoding::decode(&[0x81], Encoding::Windows1252).is_err());
}
#[test]
fn empty_input_rejected() {
    assert!(parse::parse(b"", Encoding::Cp437, &Limits::default()).is_err());
}
#[test]
fn bytes_limit() {
    let l = Limits {
        bytes: 8,
        ..Limits::default()
    };
    assert_eq!(
        parse::parse(&bytes("base.se"), Encoding::Cp437, &l)
            .unwrap_err()
            .code,
        "LIMIT_BYTES"
    );
}
#[test]
fn line_limit() {
    let l = Limits {
        line_bytes: 10,
        ..Limits::default()
    };
    assert!(parse::parse(&bytes("base.se"), Encoding::Cp437, &l).is_err());
}
#[test]
fn fields_limit() {
    assert!(parse::tokens("#X a b c", 2).is_err());
}
#[test]
fn unclosed_quote() {
    assert!(parse::tokens("#X \"a", 20).is_err());
}
#[test]
fn nested_object_rejected() {
    assert!(parse::tokens("#X {{1 A}}", 20).is_err());
}
#[test]
fn field_control_rejected() {
    assert!(parse::tokens("#X \"a\u{0}b\"", 20).is_err());
}
#[test]
fn escaped_quote() {
    assert_eq!(
        parse::tokens(r#"#X "a \"b\"""#, 20).unwrap()[1],
        Field::Text("a \"b\"".into())
    );
}
#[test]
fn base_valid() {
    assert_eq!(
        validate::validate(&doc("base.se"), &Limits::default())
            .unwrap()
            .status,
        Status::Pass
    );
}
#[test]
fn source_hash_exact() {
    assert_eq!(doc("base.se").sha256, siediff::sha256(&bytes("base.se")));
}
#[test]
fn history_not_double_counted() {
    let d = doc("base.se");
    let p = Period::new("20260101", "20260228").unwrap();
    assert_eq!(
        validate::account_movements(&d, &p).unwrap()["6540"],
        Money(150000)
    );
}
#[test]
fn rtrans_pair_required() {
    assert_eq!(
        validate::validate(&doc("unpaired-rtrans.se"), &Limits::default())
            .unwrap()
            .status,
        Status::Fail
    );
}
#[test]
fn unbalanced_voucher_fails() {
    assert_eq!(
        validate::validate(&doc("unbalanced.se"), &Limits::default())
            .unwrap()
            .status,
        Status::Fail
    );
}
#[test]
fn duplicate_scoped_identity_fails() {
    assert_eq!(
        validate::validate(&doc("duplicate-id.se"), &Limits::default())
            .unwrap()
            .status,
        Status::Fail
    );
}
#[test]
fn same_identity_different_fiscal_year_allowed() {
    assert_eq!(
        validate::validate(&doc("same-id-another-year.se"), &Limits::default())
            .unwrap()
            .status,
        Status::Pass
    );
}
#[test]
fn duplicate_balance_fails() {
    assert_eq!(
        validate::validate(&doc("duplicate-balance.se"), &Limits::default())
            .unwrap()
            .status,
        Status::Fail
    );
}
#[test]
fn unknown_is_indeterminate() {
    assert_eq!(
        validate::validate(&doc("unknown-extension.se"), &Limits::default())
            .unwrap()
            .status,
        Status::Indeterminate
    );
}
#[test]
fn extension_block_is_explicitly_unsupported() {
    assert_eq!(
        parse::parse(
            &bytes("unknown-block.se"),
            Encoding::Cp437,
            &Limits::default()
        )
        .unwrap_err()
        .code,
        "BLOCK_CONTEXT"
    );
}
#[test]
fn half_cent_is_not_rounded() {
    assert_eq!(
        parse::parse(&bytes("half-cent.se"), Encoding::Cp437, &Limits::default())
            .unwrap_err()
            .code,
        "AMOUNT_SYNTAX"
    );
}
#[test]
fn oversized_amount_refused() {
    assert_eq!(
        parse::parse(&bytes("overflow.se"), Encoding::Cp437, &Limits::default())
            .unwrap_err()
            .code,
        "AMOUNT_OVERFLOW"
    );
}
#[test]
fn invalid_source_date_refused() {
    assert!(parse::parse(
        &bytes("invalid-date.se"),
        Encoding::Cp437,
        &Limits::default()
    )
    .is_err());
}
#[test]
fn posting_batch_does_not_establish_year() {
    assert_eq!(
        validate::validate(&doc("bare-posting-batch.si"), &Limits::default())
            .unwrap()
            .status,
        Status::Indeterminate
    );
}
#[test]
fn crc_known_vector() {
    assert_eq!(parse::crc32(b"123456789"), 0xcbf43926);
}
#[test]
fn crc_fixture() {
    assert_eq!(
        doc("valid-crc.se").checksum,
        Checksum::Verified { value: 2540390227 }
    );
}
#[test]
fn crc_changed_byte_refused() {
    assert_eq!(
        parse::parse(&bytes("bad-crc.se"), Encoding::Cp437, &Limits::default())
            .unwrap_err()
            .code,
        "CHECKSUM_MISMATCH"
    );
}
#[test]
fn crc_truncation_refused() {
    assert_eq!(
        parse::parse(
            &bytes("truncated-crc.se"),
            Encoding::Cp437,
            &Limits::default()
        )
        .unwrap_err()
        .code,
        "CHECKSUM_FRAME"
    );
}
#[test]
fn whitespace_does_not_change_effects() {
    assert_eq!(
        diff("whitespace.se", Mode::Identity).status("supplied_postings"),
        Status::Pass
    );
}
#[test]
fn renumbering_requires_policy() {
    assert_eq!(
        diff("renumbered.se", Mode::Identity).status("supplied_postings"),
        Status::Fail
    );
}
#[test]
fn renumbering_under_policy() {
    let r = diff("renumbered.se", Mode::Renumbered);
    assert_eq!(r.status("supplied_postings"), Status::Pass);
    assert_eq!(r.status("voucher_identity"), Status::Fail);
}
#[test]
fn equal_finances_can_lose_history() {
    let r = diff("history-omitted.se", Mode::Identity);
    assert_eq!(r.status("supplied_postings"), Status::Pass);
    assert_eq!(r.status("retained_history"), Status::Fail);
}
#[test]
fn changed_dimensions_not_hidden_by_totals() {
    let r = diff("dimension-changed.se", Mode::Renumbered);
    assert_eq!(r.status("supplied_postings"), Status::Fail);
    assert_eq!(r.status("net_account_movements"), Status::Pass);
}
#[test]
fn changed_date_detected() {
    assert_eq!(
        diff("date-changed.se", Mode::Renumbered).status("supplied_postings"),
        Status::Fail
    );
}
#[test]
fn changed_account_detected() {
    assert_eq!(
        diff("wrong-account.se", Mode::Renumbered).status("supplied_postings"),
        Status::Fail
    );
}
#[test]
fn regrouping_is_not_silently_accepted() {
    let r = diff("regrouped.se", Mode::Renumbered);
    assert_eq!(r.status("supplied_postings"), Status::Fail);
    assert_eq!(r.status("net_account_movements"), Status::Pass);
}
#[test]
fn multiplicity_preserved() {
    assert_eq!(
        diff("duplicate-new-id.se", Mode::Renumbered).status("supplied_postings"),
        Status::Fail
    );
}
#[test]
fn wrong_entity_never_passes() {
    assert_eq!(
        diff("wrong-entity.se", Mode::Renumbered).status("supplied_postings"),
        Status::Fail
    );
}
#[test]
fn wrong_currency_never_passes() {
    assert_eq!(
        diff("wrong-currency.se", Mode::Renumbered).status("supplied_postings"),
        Status::Fail
    );
}
#[test]
fn migration_is_not_auto_approved() {
    assert_eq!(
        diff("base.se", Mode::Identity).status("migration_readiness"),
        Status::Indeterminate
    );
}
#[test]
fn accountant_is_not_auto_approved() {
    assert_eq!(
        diff("base.se", Mode::Identity).status("accountant_acceptance"),
        Status::Indeterminate
    );
}
#[test]
fn omitted_zero_is_a_declared_difference() {
    assert_eq!(
        diff("missing-zero-control.se", Mode::Identity).status("declared_balances"),
        Status::Fail
    );
}
#[test]
fn no_vacuous_posting_pass() {
    let d = doc("empty-postings.se");
    let r = compare::compare(&d, &d, &Policy::default(), &Limits::default()).unwrap();
    assert_eq!(r.status("supplied_postings"), Status::NotApplicable);
}
#[test]
fn ambiguous_groups_not_arbitrarily_paired() {
    let r = compare::compare(
        &doc("ambiguous-left.se"),
        &doc("ambiguous-right.se"),
        &Policy {
            mode: Mode::Renumbered,
            ..Policy::default()
        },
        &Limits::default(),
    )
    .unwrap();
    assert_eq!(r.status("supplied_postings"), Status::Pass);
    assert_eq!(r.status("unique_correspondence"), Status::Indeterminate);
    assert!(r
        .groups
        .iter()
        .all(|g| g.left_count == 2 && g.right_count == 2 && !g.unique_correspondence));
}
#[test]
fn deterministic_replay() {
    let x = serde_json::to_vec(&diff("base.se", Mode::Identity)).unwrap();
    let y = serde_json::to_vec(&diff("base.se", Mode::Identity)).unwrap();
    assert_eq!(x, y);
}
#[test]
fn comparison_preserves_originals() {
    let a = bytes("base.se");
    let _ = diff("date-changed.se", Mode::Identity);
    assert_eq!(bytes("base.se"), a);
}
#[test]
fn mapped_accounts_work() {
    let m: Mapping = strict_json::from_slice(&bytes("account-mapping.json")).unwrap();
    let r = compare::compare(
        &doc("base.se"),
        &doc("mapped-account.se"),
        &Policy {
            mapping: Some(m),
            ..Policy::default()
        },
        &Limits::default(),
    )
    .unwrap();
    assert_eq!(r.status("supplied_postings"), Status::Pass);
}
#[test]
fn stale_mapping_refused() {
    let mut m: Mapping = strict_json::from_slice(&bytes("account-mapping.json")).unwrap();
    m.left_sha256 = "0".repeat(64);
    assert_eq!(
        compare::compare(
            &doc("base.se"),
            &doc("mapped-account.se"),
            &Policy {
                mapping: Some(m),
                ..Policy::default()
            },
            &Limits::default()
        )
        .unwrap_err()
        .code,
        "MAPPING_BASIS"
    );
}
#[test]
fn mapping_collisions_refused() {
    let a = doc("base.se");
    let m = Mapping {
        schema: "siediff.mapping.v1".into(),
        left_sha256: a.sha256.clone(),
        right_sha256: a.sha256.clone(),
        review_ref: "fiction".into(),
        accounts: BTreeMap::from([("6540".into(), "6570".into())]),
        dimensions: BTreeMap::new(),
        objects: BTreeMap::new(),
    };
    assert!(compare::compare(
        &a,
        &a,
        &Policy {
            mapping: Some(m),
            ..Policy::default()
        },
        &Limits::default()
    )
    .is_err());
}
#[test]
fn json_duplicate_keys_refused() {
    assert!(strict_json::from_slice::<serde_json::Value>(br#"{"a":1,"a":2}"#).is_err());
}
#[test]
fn nested_duplicate_keys_refused() {
    assert!(strict_json::from_slice::<serde_json::Value>(br#"{"x":{"a":1,"a":2}}"#).is_err());
}
#[test]
fn snapshot_matches_supplied_effects() {
    let b = snapshot::parse(&bytes("base.snapshot.json"), &Limits::default()).unwrap();
    let r = compare::compare(&doc("base.se"), &b, &Policy::default(), &Limits::default()).unwrap();
    assert_eq!(r.status("supplied_postings"), Status::Pass);
}
#[test]
fn posted_snapshot_cannot_claim_full_history() {
    let b = snapshot::parse(&bytes("base.posted.snapshot.json"), &Limits::default()).unwrap();
    let r = compare::compare(&doc("base.se"), &b, &Policy::default(), &Limits::default()).unwrap();
    assert_eq!(r.status("supplied_postings"), Status::Pass);
    assert_eq!(r.status("retained_history"), Status::Indeterminate);
}
#[test]
fn independent_controls_pass_on_authored_base() {
    let m: control::Manifest = strict_json::from_slice(&bytes("base.controls.json")).unwrap();
    assert_eq!(
        control::check(&doc("base.se"), &m, &Limits::default())
            .unwrap()
            .status,
        Status::Pass
    );
}
#[test]
fn independent_controls_expose_compensating_error() {
    let m: control::Manifest =
        strict_json::from_slice(&bytes("missing-payment-compensated.controls.json")).unwrap();
    let r = control::check(
        &doc("missing-payment-compensated.se"),
        &m,
        &Limits::default(),
    )
    .unwrap();
    let states: BTreeMap<_, _> = r
        .observations
        .iter()
        .map(|x| (x.id.as_str(), x.status))
        .collect();
    assert_eq!(states["bank-opening"], Status::Fail);
    assert_eq!(states["bank-movement"], Status::Fail);
    assert_eq!(states["bank-closing"], Status::Pass);
    assert_eq!(states["bank-bridge"], Status::Pass);
}
#[test]
fn absent_control_not_zero() {
    let m: control::Manifest =
        strict_json::from_slice(&bytes("missing-zero-control.controls.json")).unwrap();
    assert_eq!(
        control::check(&doc("missing-zero-control.se"), &m, &Limits::default())
            .unwrap()
            .status,
        Status::Indeterminate
    );
}
#[test]
fn zero_omission_requires_explicit_premise() {
    let m: control::Manifest =
        strict_json::from_slice(&bytes("missing-zero-control-zero.controls.json")).unwrap();
    assert_eq!(
        control::check(&doc("missing-zero-control.se"), &m, &Limits::default())
            .unwrap()
            .status,
        Status::Pass
    );
}
#[test]
fn coverage_unknown_cannot_pass() {
    let mut m: control::Manifest = strict_json::from_slice(&bytes("base.controls.json")).unwrap();
    m.coverage = control::Coverage::Unknown;
    assert_eq!(
        control::check(&doc("base.se"), &m, &Limits::default())
            .unwrap()
            .status,
        Status::Indeterminate
    );
}
#[test]
fn stale_controls_refused() {
    let m: control::Manifest = strict_json::from_slice(&bytes("base.controls.json")).unwrap();
    assert!(control::check(
        &doc("missing-payment-compensated.se"),
        &m,
        &Limits::default()
    )
    .is_err());
}
#[test]
fn bridge_must_expect_zero() {
    let mut m: control::Manifest = strict_json::from_slice(&bytes("base.controls.json")).unwrap();
    m.controls.last_mut().unwrap().expected = Money(1);
    assert_eq!(
        control::check(&doc("base.se"), &m, &Limits::default())
            .unwrap_err()
            .code,
        "CONTROL_BRIDGE"
    );
}
#[test]
fn truncated_diagnostics_do_not_turn_failure_into_pass() {
    let l = Limits {
        findings: 0,
        ..Limits::default()
    };
    let v = validate::validate(&doc("unbalanced.se"), &l).unwrap();
    assert_eq!(v.status, Status::Fail);
    assert!(v.findings_truncated);
}
#[test]
fn malformed_input_campaign_does_not_panic() {
    let base = bytes("base.se");
    let mut seed = 42u64;
    for _ in 0..2000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut b = base.clone();
        let i = (seed as usize) % b.len();
        b[i] = (seed >> 32) as u8;
        let outcome =
            std::panic::catch_unwind(|| parse::parse(&b, Encoding::Cp437, &Limits::default()));
        assert!(outcome.is_ok());
    }
}
#[test]
fn changed_identity_has_row_locations() {
    let r = diff("dimension-changed.se", Mode::Identity);
    assert!(r
        .differences
        .iter()
        .any(|d| d.code == "SAME_ID_POSTINGS_CHANGED"));
    assert!(r
        .differences
        .iter()
        .any(|d| d.code == "ROW_MULTISET_DIFFERENCE"));
}
#[test]
fn selected_period_does_not_split_rows() {
    let mut d = doc("history-omitted.se");
    d.vouchers[0].rows[0].date = Some("20260201".into());
    let p = Policy {
        mode: Mode::Identity,
        period: Some(Period::new("20260101", "20260131").unwrap()),
        mapping: None,
    };
    let r = compare::compare(&d, &d, &p, &Limits::default()).unwrap();
    assert_eq!(r.status("supplied_postings"), Status::Indeterminate);
}
