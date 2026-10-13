#!/usr/bin/env python3
"""Public CLI regressions authored before extensions; retain actual outcomes."""
import copy
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(sys.argv[1]).resolve()
OUTPUT = Path(sys.argv[2]).resolve()
OUTPUT.mkdir(parents=True, exist_ok=True)
results = []
commands = []
(OUTPUT / "cases").mkdir(exist_ok=True)

def run(args, expected_exit):
    p = subprocess.run([str(BINARY), *map(str, args)], capture_output=True, timeout=20)
    commands.append({'arguments': list(map(str,args)), 'exit': p.returncode, 'expected_exit': expected_exit, 'stdout': p.stdout.decode(), 'stderr': p.stderr.decode()})
    assert p.returncode == expected_exit, (p.returncode, p.stderr.decode())
    return json.loads(p.stdout) if p.stdout else None

def claim(report, name):
    return next(a['status'] for a in report['assertions'] if a['id'] == name)

with tempfile.TemporaryDirectory(prefix='siediff-cli-') as scratch:
    scratch = Path(scratch)
    def save(name, data):
        p = scratch / name
        p.write_text(json.dumps(data))
        (OUTPUT / "cases" / name).write_bytes(p.read_bytes())
        return p
    original = json.loads((ROOT / 'fixtures/base.posted.snapshot.json').read_text())
    left = save('left.json', original)
    def compare(right, code=1, extra=()):
        return run(['diff', left, right, '--left-format', 'snapshot', '--right-format', 'snapshot', '--json', *extra], code)
    def fields():
        changed = copy.deepcopy(original)
        changed['vouchers'][1]['rows'][0]['amount'] = '90000'
        changed['vouchers'][1]['rows'][1]['amount'] = '-90000'
        r = compare(save('amount.json', changed))
        assert {(x['before'], x['after']) for x in r['explanations'] if x['field'] == 'amount'} == {('100000', '90000'), ('-100000', '-90000')}
    def revised_credit():
        changed = copy.deepcopy(original)
        changed['vouchers'][2]['rows'][0]['amount'] = '50000'
        changed['vouchers'][2]['rows'][1]['amount'] = '-40000'
        changed['vouchers'][2]['rows'][2]['amount'] = '-10000'
        r = compare(save('revised-credit.json', changed))
        assert claim(r, 'supplied_postings') == 'fail'
        assert claim(r, 'migration_readiness') == 'indeterminate'
        assert {(x['before'], x['after']) for x in r['explanations'] if x['field'] == 'amount'} == {('62500','50000'),('-50000','-40000'),('-12500','-10000')}
    def ambiguity():
        s = copy.deepcopy(original)
        v = copy.deepcopy(s['vouchers'][0])
        v['number'] = '99'
        s['vouchers'].append(v)
        p = save('ambiguity.json', s)
        r = run(['diff', p, p, '--left-format', 'snapshot', '--right-format', 'snapshot', '--mode', 'renumbered', '--json'], 0)
        assert claim(r, 'unique_correspondence') == 'indeterminate'
        assert any(e['field'] == 'correspondence' and e['certainty'] == 'unresolved' for e in r['explanations'])
    def relationships():
        s = copy.deepcopy(original)
        s['basis']['native_relationships'] = {'coverage': 'complete_asserted', 'links': [{'voucher': '20260101:20261231:A:3', 'corrects': '20260101:20261231:A:1'}]}
        a = save('links-a.json', s)
        t = copy.deepcopy(s)
        t['basis']['native_relationships']['links'] = []
        b = save('links-b.json', t)
        r = run(['diff', a, b, '--left-format', 'snapshot', '--right-format', 'snapshot', '--claim', 'native_relationships', '--json'], 1)
        assert claim(r, 'supplied_postings') == 'pass'
        assert claim(r, 'native_relationships') == 'fail'
        t['basis']['native_relationships']['links'] = [{'voucher': '20260101:20261231:A:3', 'corrects': 'absent'}]
        run(['validate', save('dangling.json', t), '--format', 'snapshot'], 2)
        t['basis']['native_relationships']['links'] = [{'voucher': '20260101:20261231:A:3', 'corrects': '20260101:20261231:A:1'}, {'voucher': '20260101:20261231:A:1', 'corrects': '20260101:20261231:A:3'}]
        run(['validate', save('cycle.json', t), '--format', 'snapshot'], 2)
    def impact():
        right = save('right.json', original)
        sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
        basis = {'schema': 'siediff.work-basis.v1', 'left_sha256': sha(left), 'right_sha256': sha(right), 'book_id': 'fictional-book', 'before_revision': 'fixture-revision-001', 'after_revision': 'fixture-revision-001', 'decisions': [
            {'id': 'match-1', 'state': 'approved', 'assertions': ['supplied_postings'], 'inventories': [{'id': 'eligible-invoices', 'before': 'a'*64, 'after': 'b'*64, 'coverage': 'complete_asserted'}], 'requirements': []},
            {'id': 'unrelated', 'state': 'prepared', 'assertions': ['supplied_postings'], 'inventories': [], 'requirements': []},
            {'id': 'missing-evidence', 'state': 'prepared', 'assertions': ['supplied_postings'], 'inventories': [], 'requirements': [{'id': 'source-original', 'state': 'unknown', 'route': 'request_evidence'}]}]}
        r = compare(right, 0, ['--work-basis', save('work.json', basis)])
        work = {x['decision_id']: x for x in r['required_work']}
        assert work['match-1']['action'] == 'requalify_approval'
        assert work['unrelated']['action'] == 'retain_support'
        assert work['missing-evidence']['action'] == 'resolve_requirements'
        assert not any(x['authorizes_execution'] for x in work.values())
        basis['decisions'][0]['state'] = 'posted'
        r = compare(right, 0, ['--work-basis', save('posted-work.json', basis)])
        assert r['required_work'][0]['action'] == 'review_correction'
        basis['left_sha256'] = 'f'*64
        run(['diff', left, right, '--left-format', 'snapshot', '--right-format', 'snapshot', '--work-basis', save('stale.json', basis)], 2)
    def monthly_objects():
        sha = hashlib.sha256(left.read_bytes()).hexdigest()
        manifest = {'schema': 'siediff.controls.v1', 'ledger_sha256': sha, 'organization': original['organization'], 'organization_parts': [], 'currency': 'SEK', 'fiscal_period': {'start': '20260101', 'end': '20261231'}, 'through': '20260228', 'coverage': 'complete_asserted', 'omission_means_zero': False, 'review_ref': 'independent fictional arithmetic', 'controls': [
          {'id': 'jan-object', 'kind': 'movement', 'from': '20260101', 'through': '20260131', 'account': '6540', 'objects': [{'dimension': '1', 'object': 'A'}], 'expected': '200000', 'evidence_sha256': 'a'*64, 'evidence_locator': 'authored January invoice'},
          {'id': 'feb-object', 'kind': 'movement', 'from': '20260201', 'through': '20260228', 'account': '6540', 'objects': [{'dimension': '1', 'object': 'A'}], 'expected': '-50000', 'evidence_sha256': 'b'*64, 'evidence_locator': 'authored February credit'}]}
        r = run(['controls', left, save('monthly.json', manifest), '--format', 'snapshot', '--json'], 0)
        assert [x['observed'] for x in r['observations']] == ['200000', '-50000']
        manifest['controls'][1]['through'] = '20260131'
        run(['controls', left, save('invalid-window.json', manifest), '--format', 'snapshot'], 2)
    def controls():
        r = run(['controls', ROOT/'fixtures/missing-payment-compensated.se', ROOT/'fixtures/missing-payment-compensated.controls.json', '--json'], 1)
        assert r['status'] == 'fail'
        assert any(o['status'] == 'fail' for o in r['observations'])
    for name, fn in [('exact field explanations', fields), ('revised credit', revised_credit), ('ambiguous duplicates', ambiguity), ('native correction links', relationships), ('candidate inventory and requirements', impact), ('compensated omitted payment', controls), ('monthly object controls', monthly_objects)]:
        try:
            fn()
            results.append({'case': name, 'passed': True})
        except (AssertionError, KeyError) as exc:
            results.append({'case': name, 'passed': False, 'reason': str(exc)})

(OUTPUT/'cli-journeys.json').write_text(json.dumps({'passed': all(x['passed'] for x in results), 'cases': results, 'commands': commands}, indent=2)+'\n')
print(json.dumps(results, indent=2))
raise SystemExit(0 if all(x['passed'] for x in results) else 1)
