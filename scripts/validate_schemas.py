#!/usr/bin/env python3
from pathlib import Path
import json
from jsonschema import Draft202012Validator
R=Path(__file__).resolve().parents[1]
def read(p):return json.loads(p.read_text())
schemas={p.name.split('.')[0]:read(p) for p in (R/'schemas').glob('*.schema.json')}
for s in schemas.values():Draft202012Validator.check_schema(s)
validated=[]
for p in (R/'fixtures').glob('*.json'):
 o=read(p);tag=o.get('schema','')
 name={'siediff.snapshot.v1':'snapshot','siediff.controls.v1':'controls','siediff.mapping.v1':'mapping'}.get(tag)
 if name:Draft202012Validator(schemas[name]).validate(o);validated.append(p.name)
# Negative contract tests, especially numeric money.
bad=read(R/'fixtures/base.snapshot.json');bad['vouchers'][0]['rows'][0]['amount']=200000
assert list(Draft202012Validator(schemas['snapshot']).iter_errors(bad))
bad=read(R/'fixtures/base.controls.json');bad['controls'][0]['expected']=2000000
assert list(Draft202012Validator(schemas['controls']).iter_errors(bad))
out={'schemas':len(schemas),'validated_fixtures':validated,'negative_tests':2,'passed':True}
print(json.dumps(out))
