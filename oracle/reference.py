"""Independent, deliberately small executable oracle for the authored corpus.
It is not the Rust implementation and is not offered as a production SIE parser.
Uses Python Decimal, tuples and multisets instead of the Rust implementation's
integer accumulator, typed records and JSON grouping.
"""
from collections import Counter, defaultdict
from decimal import Decimal
from datetime import datetime
import re, zlib, hashlib
TOKEN=re.compile(r'"(?:\\.|[^"\\])*"|[{}]|[^\s{}"]+')
MONEY=re.compile(r'-?[0-9]+(?:\.[0-9]{1,2})?\Z')
LOW=-(2**127); HIGH=2**127-1

def money(s):
    if not MONEY.fullmatch(s): raise ValueError('AMOUNT_SYNTAX')
    # Decimal's context precision must not affect string-to-integer conversion.
    sign,digits,exp=Decimal(s).as_tuple()
    n=int(''.join(map(str,digits)) or '0')*10**(exp+2)
    n=-n if sign else n
    if not LOW<=n<=HIGH: raise ValueError('AMOUNT_OVERFLOW')
    return n

def lex_line(s):
    result=[];end=0
    for m in TOKEN.finditer(s):
        if s[end:m.start()].strip():raise ValueError('TOKEN')
        v=m[0]
        if v.startswith('"'):v=v[1:-1].replace('\\"','"')
        result.append(v);end=m.end()
    if s[end:].strip():raise ValueError('TOKEN')
    return result

def lex(data,encoding='cp437'):
    records=[];offset=0
    for number,raw in enumerate(data.splitlines(keepends=True),1):
        s=raw.decode(encoding).rstrip('\r\n').strip(' \t')
        if s:
            t=lex_line(s)
            fields=[];i=1
            while i<len(t):
                if t[i]=='{':
                    j=t.index('}',i+1); fields.append(t[i+1:j]);i=j+1
                else:fields.append(t[i]);i+=1
            records.append({'tag':t[0],'fields':fields,'line':number,'start':offset,'end':offset+len(raw),'record':len(records)})
        offset+=len(raw)
    return records

def crc_payload(records):
    pieces=[]
    for r in records:
        if r['tag'] in ['{','}']:continue
        pieces.append(r['tag'])
        for f in r['fields']:pieces.extend(f if isinstance(f,list) else [f])
    return ''.join(pieces).encode('cp437')

def parse(data):
    records=lex(data);vouchers=[];balances=[];years={};accounts={};dimensions={};objects=[];current=None;issues=[]
    organization=None;currency='SEK';extent=None;name=None;type_=1
    checksum=[i for i,r in enumerate(records) if r['tag']=='#KSUMMA']
    if checksum:
        if len(checksum)!=2 or checksum[0]!=1 or checksum[1]!=len(records)-1:raise ValueError('CHECKSUM_FRAME')
        if zlib.crc32(crc_payload(records[checksum[0]+1:checksum[1]]))!=int(records[checksum[1]]['fields'][0]):raise ValueError('CHECKSUM_MISMATCH')
    for r in records:
        tag=r['tag']; f=r['fields']
        if tag=='#ORGNR':organization=f[0]
        elif tag=='#VALUTA':currency=f[0]
        elif tag=='#FNAMN':name=f[0]
        elif tag=='#OMFATTN':extent=f[0]
        elif tag=='#SIETYP':type_=int(f[0])
        elif tag=='#RAR':
            y=int(f[0]);p=(f[1],f[2]);datetime.strptime(p[0],'%Y%m%d');datetime.strptime(p[1],'%Y%m%d')
            if y in years:raise ValueError('DUPLICATE_YEAR')
            years[y]=p
        elif tag=='#KONTO':
            if f[0] in accounts:raise ValueError('DUPLICATE_ACCOUNT')
            accounts[f[0]]={'number':f[0],'name':f[1],'kind':None,'unit':None}
        elif tag=='#KTYP':accounts[f[0]]['kind']=f[1]
        elif tag=='#ENHET':accounts[f[0]]['unit']=f[1]
        elif tag in ['#DIM','#UNDERDIM']:dimensions[f[0]]={'id':f[0],'name':f[1],'parent':f[2] if len(f)>2 else None}
        elif tag=='#OBJEKT':objects.append({'dimension':f[0],'number':f[1],'name':f[2]})
        elif tag=='#VER':
            datetime.strptime(f[2],'%Y%m%d')
            current={'series':f[0],'number':f[1],'date':f[2],'text':f[3] if len(f)>3 and f[3] else None,'registered_on':f[4] if len(f)>4 and f[4] else None,'signature':f[5] if len(f)>5 and f[5] else None,'rows':[],'loc':r}
        elif tag in ['#TRANS','#RTRANS','#BTRANS']:
            if current is None:raise ValueError('ROW_CONTEXT')
            date=f[3] if len(f)>3 and f[3] else None
            if date:datetime.strptime(date,'%Y%m%d')
            ob=tuple(sorted(zip(f[1][::2],f[1][1::2])))
            current['rows'].append({'kind':tag,'account':f[0],'objects':ob,'amount':money(f[2]),'date':date,'text':f[4] if len(f)>4 and f[4] else None,'quantity':f[5] if len(f)>5 and f[5] else None,'signature':f[6] if len(f)>6 and f[6] else None,'loc':r})
        elif tag=='}':
            if current is None:raise ValueError('BLOCK_CONTEXT')
            vouchers.append(current);current=None
        elif tag in ['#IB','#UB','#RES','#OIB','#OUB','#PSALDO','#PBUDGET']:
            if tag in ['#IB','#UB','#RES']:a=1;oi=None;vi=2;mo=None
            elif tag in ['#OIB','#OUB']:a=1;oi=2;vi=3;mo=None
            else:a=2;oi=3;vi=4;mo=f[1]
            ob=tuple(sorted(zip(f[oi][::2],f[oi][1::2]))) if oi is not None else ()
            balances.append({'tag':tag,'year':int(f[0]),'month':mo,'account':f[a],'objects':ob,'amount':money(f[vi]),'quantity':f[vi+1] if len(f)>vi+1 and f[vi+1] else None,'loc':r})
        elif tag.startswith('#VENDOR'):issues.append('UNINTERPRETED_RECORDS')
    if current is not None:raise ValueError('TRUNCATED_VOUCHER')
    d={'records':records,'years':years,'accounts':accounts,'dimensions':dimensions,'objects':objects,'vouchers':vouchers,'balances':balances,'organization':organization,'currency':currency,'extent':extent,'name':name,'type':type_,'issues':issues}
    ids=set()
    for v in vouchers:
        p=period(d,v['date']);identity=(p,v['series'],v['number'])
        if v['series'] and v['number']:
            if identity in ids:issues.append('VOUCHER_ID_DUPLICATE')
            ids.add(identity)
        if sum(r['amount'] for r in v['rows'] if r['kind']=='#TRANS'):issues.append('VOUCHER_UNBALANCED')
        for i,r in enumerate(v['rows']):
            if r['kind']=='#RTRANS':
                if i+1==len(v['rows']) or v['rows'][i+1]['kind']!='#TRANS' or row(r,v,history=True)!=row(v['rows'][i+1],v,history=True):issues.append('RTRANS_PAIR')
    seen=set()
    for b in balances:
        k=(b['tag'],b['year'],b['month'],b['account'],b['objects'])
        if k in seen:issues.append('BALANCE_DUPLICATE')
        seen.add(k)
    return d

def period(d,date):
    options=[p for p in d['years'].values() if p[0]<=date<=p[1]]
    return options[0] if len(options)==1 else None

def row(r,v,history=False):
    if history:return(r['account'],r['objects'],r['amount'],r['date'],r['text'],r['quantity'],r['signature'])
    return(r['account'],r['objects'],r['amount'],r['date'] or v['date'],r['quantity'])

def financial(d,v):return(period(d,v['date']),v['date'],tuple(sorted(row(r,v) for r in v['rows'] if r['kind']=='#TRANS')))
def history(v):return(v['date'],v['text'],v['registered_on'],v['signature'],tuple((r['kind'],row(r,v,True)) for r in v['rows']))
def compare(a,b,renumbered=False):
    fa=Counter(financial(a,v) for v in a['vouchers']);fb=Counter(financial(b,v) for v in b['vouchers'])
    ia=Counter((period(a,v['date']),v['series'],v['number'],financial(a,v)) for v in a['vouchers']);ib=Counter((period(b,v['date']),v['series'],v['number'],financial(b,v)) for v in b['vouchers'])
    ma=defaultdict(int);mb=defaultdict(int)
    for d,m in [(a,ma),(b,mb)]:
        for v in d['vouchers']:
            for r in v['rows']:
                if r['kind']=='#TRANS':m[r['account']]+=r['amount']
    history_a=Counter((financial(a,v),history(v)) for v in a['vouchers']);history_b=Counter((financial(b,v),history(v)) for v in b['vouchers'])
    valid=not a['issues'] and not b['issues']
    same_scope=a['organization']==b['organization'] and a['organization'] is not None and a['currency']==b['currency']
    return {'financial_values_equal':fa==fb,'identity_equal':ia==ib,'postings_equal':valid and same_scope and fa==fb and (renumbered or ia==ib),'history_equal':history_a==history_b,'net_equal':dict(ma)==dict(mb),'valid':valid,'unique_groups':all(n==1 and fb[k]==1 for k,n in fa.items()),'empty':not fa and not fb}

def controls(d):
    opening=next((b['amount'] for b in d['balances'] if b['tag']=='#IB' and b['year']==0 and b['account']=='1930'),None)
    closing=next((b['amount'] for b in d['balances'] if b['tag']=='#UB' and b['year']==0 and b['account']=='1930'),None)
    movement=sum(r['amount'] for v in d['vouchers'] for r in v['rows'] if r['kind']=='#TRANS' and r['account']=='1930')
    # Authored independent controls, not inferred from this export.
    return {'opening_ok':opening==2000000,'closing_ok':closing==1812500,'movement_ok':movement==-187500,'bridge_ok':opening+movement==closing}
