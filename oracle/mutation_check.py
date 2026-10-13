"""Wrong-checker challenge at specification level. NOT Rust mutation coverage."""
from pathlib import Path
import json
from reference import parse,compare,financial,controls
F=Path(__file__).resolve().parents[1]/'fixtures'
a=parse((F/'base.se').read_bytes())
def case(n):return parse((F/n).read_bytes())
# Each intentionally bad rule is checked against an independently chosen witness.
checks=[
 ('bytes_only','whitespace.se',False,True),
 ('net_totals_only','dimension-changed.se',compare(a,case('dimension-changed.se'))['net_equal'],False),
 ('ignore_dates','date-changed.se',compare(a,case('date-changed.se'))['net_equal'],False),
 ('drop_voucher_grouping','regrouped.se',compare(a,case('regrouped.se'))['net_equal'],False),
 ('deduplicate_vouchers','duplicate-new-id.se',set(financial(a,v) for v in a['vouchers'])==set(financial(case('duplicate-new-id.se'),v) for v in case('duplicate-new-id.se')['vouchers']),False),
 ('ignore_correction_history','history-omitted.se',compare(a,case('history-omitted.se'))['postings_equal'],False),
 ('ignore_entity','wrong-entity.se',compare(a,case('wrong-entity.se'))['financial_values_equal'],False),
 ('ignore_currency','wrong-currency.se',compare(a,case('wrong-currency.se'))['financial_values_equal'],False),
 ('ignore_rtrans_pair_validation','unpaired-rtrans.se',compare(a,case('unpaired-rtrans.se'))['financial_values_equal'],False),
 ('always_pass','wrong-account.se',True,False),
 ('always_fail','base.se',False,True),
 ('internal_bridge_is_enough','missing-payment-compensated.se',controls(case('missing-payment-compensated.se'))['bridge_ok'],False),
]
results=[{'mutant':n,'witness':w,'wrong_result':x,'independent_expected':y,'killed':x!=y} for n,w,x,y in checks]
assert all(r['killed'] for r in results)
print(json.dumps({'wrong_checker_hypotheses_rejected':len(results),'rust_mutants_executed':0,'passed':True}))
