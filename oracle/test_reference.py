import unittest,random,hashlib,zlib,json
from pathlib import Path
from reference import *
F=Path(__file__).resolve().parents[1]/'fixtures'
def load(n):return parse((F/n).read_bytes())
class IndependentExpectations(unittest.TestCase):
    def setUp(self):self.base=load('base.se')
    def test_base_final_money(self):
        self.assertEqual(controls(self.base),dict(opening_ok=True,closing_ok=True,movement_ok=True,bridge_ok=True))
    def test_corruption_is_balanced(self):
        d=load('missing-payment-compensated.se');self.assertNotIn('VOUCHER_UNBALANCED',d['issues'])
    def test_independent_opening_detects_hidden_missing_payment(self):
        self.assertEqual(controls(load('missing-payment-compensated.se')),dict(opening_ok=False,closing_ok=True,movement_ok=False,bridge_ok=True))
    def test_renumbering_is_not_identity(self):
        c=compare(self.base,load('renumbered.se'));self.assertTrue(c['financial_values_equal']);self.assertFalse(c['identity_equal']);self.assertFalse(c['postings_equal'])
    def test_renumbering_explicitly_allowed(self):self.assertTrue(compare(self.base,load('renumbered.se'),True)['postings_equal'])
    def test_formatting_only(self):self.assertTrue(compare(self.base,load('whitespace.se'))['postings_equal'])
    def test_history_loss_not_financial_loss(self):
        c=compare(self.base,load('history-omitted.se'));self.assertTrue(c['postings_equal']);self.assertFalse(c['history_equal'])
    def test_dimensions_are_financial_information(self):
        c=compare(self.base,load('dimension-changed.se'));self.assertTrue(c['net_equal']);self.assertFalse(c['postings_equal'])
    def test_date_change(self):self.assertFalse(compare(self.base,load('date-changed.se'))['postings_equal'])
    def test_wrong_account(self):self.assertFalse(compare(self.base,load('wrong-account.se'))['postings_equal'])
    def test_regrouping_is_not_voucher_equivalence(self):
        c=compare(self.base,load('regrouped.se'),True);self.assertTrue(c['net_equal']);self.assertFalse(c['postings_equal'])
    def test_duplicate_identity_is_invalid(self):self.assertIn('VOUCHER_ID_DUPLICATE',load('duplicate-id.se')['issues'])
    def test_duplicate_under_new_id_still_differs(self):self.assertFalse(compare(self.base,load('duplicate-new-id.se'),True)['postings_equal'])
    def test_unpaired_rtrans(self):self.assertIn('RTRANS_PAIR',load('unpaired-rtrans.se')['issues'])
    def test_unbalanced(self):self.assertIn('VOUCHER_UNBALANCED',load('unbalanced.se')['issues'])
    def test_unknown_is_not_pass(self):self.assertFalse(compare(self.base,load('unknown-extension.se'))['postings_equal'])
    def test_no_double_counting_history(self):
        self.assertEqual(sum(r['amount'] for v in self.base['vouchers'] for r in v['rows'] if r['kind']=='#TRANS' and r['account']=='6540'),150000)
    def test_half_cent_rejected(self):
        with self.assertRaises(ValueError):load('half-cent.se')
    def test_overflow_rejected(self):
        with self.assertRaises(ValueError):load('overflow.se')
    def test_invalid_gregorian_date(self):
        with self.assertRaises(ValueError):load('invalid-date.se')
    def test_duplicate_balance(self):self.assertIn('BALANCE_DUPLICATE',load('duplicate-balance.se')['issues'])
    def test_scope_entity(self):self.assertFalse(compare(self.base,load('wrong-entity.se'))['postings_equal'])
    def test_scope_currency(self):self.assertFalse(compare(self.base,load('wrong-currency.se'))['postings_equal'])
    def test_scoped_identity_across_years(self):self.assertNotIn('VOUCHER_ID_DUPLICATE',load('same-id-another-year.se')['issues'])
    def test_many_to_many_ambiguity(self):
        c=compare(load('ambiguous-left.se'),load('ambiguous-right.se'),True);self.assertTrue(c['postings_equal']);self.assertFalse(c['unique_groups'])
    def test_no_transactions_not_evidence(self):self.assertTrue(compare(load('empty-postings.se'),load('empty-postings.se'))['empty'])
    def test_money_exact_random(self):
        rng=random.Random(1930)
        for _ in range(20000):
            n=rng.randrange(-10**25,10**25);s=('-' if n<0 else '')+str(abs(n)//100)+'.'+str(abs(n)%100).zfill(2);self.assertEqual(money(s),n)
    def test_invalid_money_forms(self):
        for s in ['','+1','1,00','1e2','NaN','1.001','1.','--1',' 1','1_000']:
            with self.subTest(s=s),self.assertRaises(ValueError):money(s)
    def test_crc_reference_vector(self):self.assertEqual(zlib.crc32(b'123456789'),0xcbf43926)
    def test_fixture_byte_hashes(self):
        m=json.loads((F/'base.controls.json').read_text());self.assertEqual(m['ledger_sha256'],hashlib.sha256((F/'base.se').read_bytes()).hexdigest())
    def test_schema_money_is_integer_string(self):
        m=json.loads((F/'base.controls.json').read_text());self.assertTrue(all(isinstance(c['expected'],str) for c in m['controls']))
    def test_source_bank_independent_expected(self):
        b=json.loads((F/'independent-bank.json').read_text());self.assertEqual(int(b['opening_minor'])+sum(map(int,b['movements_minor'])),int(b['closing_minor']))
if __name__=='__main__':unittest.main(verbosity=2)
