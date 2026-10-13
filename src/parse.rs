use crate::{
    encoding,
    model::*,
    money::{quantity, Money},
    sha256, Error, Result,
};
use std::collections::{BTreeMap, BTreeSet};

pub fn tokens(s: &str, max: usize) -> Result<Vec<Field>> {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = vec![];
    while i < c.len() {
        while i < c.len() && matches!(c[i], ' ' | '\t') {
            i += 1
        }
        if i == c.len() {
            break;
        }
        if out.len() >= max {
            return Err(Error::new("LIMIT_FIELDS", "Too many fields"));
        }
        if c[i] == '{' {
            i += 1;
            let mut obj = vec![];
            loop {
                while i < c.len() && matches!(c[i], ' ' | '\t') {
                    i += 1
                }
                if i == c.len() {
                    return Err(Error::new("OBJECT_UNCLOSED", "Unclosed object list"));
                }
                if c[i] == '}' {
                    i += 1;
                    break;
                }
                if c[i] == '{' {
                    return Err(Error::new("OBJECT_NESTED", "Nested object list"));
                }
                if obj.len() >= max {
                    return Err(Error::new("LIMIT_FIELDS", "Too many object fields"));
                }
                obj.push(token(&c, &mut i)?);
            }
            if i < c.len() && !matches!(c[i], ' ' | '\t') {
                return Err(Error::new("FIELD_SEPARATOR", "Missing field separator"));
            }
            out.push(Field::Objects(obj));
        } else if c[i] == '}' {
            return Err(Error::new("BRACE", "Unexpected brace"));
        } else {
            out.push(Field::Text(token(&c, &mut i)?))
        }
    }
    Ok(out)
}
fn token(c: &[char], i: &mut usize) -> Result<String> {
    let mut s = String::new();
    if c[*i] == '"' {
        *i += 1;
        let mut closed = false;
        while *i < c.len() {
            let b = c[*i];
            if b == '\\' && c.get(*i + 1) == Some(&'"') {
                s.push('"');
                *i += 2;
                continue;
            }
            if b == '"' {
                *i += 1;
                closed = true;
                break;
            }
            if b.is_control() {
                return Err(Error::new("TEXT_CONTROL", "Control character in field"));
            }
            s.push(b);
            *i += 1;
        }
        if !closed {
            return Err(Error::new("QUOTE_UNCLOSED", "Unclosed quoted field"));
        }
        if *i < c.len() && !matches!(c[*i], ' ' | '\t' | '}') {
            return Err(Error::new("FIELD_SEPARATOR", "Missing field separator"));
        }
    } else {
        while *i < c.len() && !matches!(c[*i], ' ' | '\t' | '}') {
            let b = c[*i];
            if b == '{' || b == '"' || b.is_control() {
                return Err(Error::new("TOKEN", "Invalid unquoted token"));
            }
            s.push(b);
            *i += 1;
        }
        if s.is_empty() {
            return Err(Error::new("TOKEN", "Empty unquoted token"));
        }
    }
    Ok(s)
}
pub fn lexical(bytes: &[u8], enc: Encoding, l: &Limits) -> Result<Vec<Record>> {
    if bytes.len() > l.bytes {
        return Err(Error::new("LIMIT_BYTES", "Input exceeds byte limit"));
    }
    if bytes.starts_with(b"\xef\xbb\xbf") && enc != Encoding::Utf8 {
        return Err(Error::new(
            "ENCODING_BOM",
            "UTF-8 BOM requires explicit UTF-8 profile",
        ));
    }
    let mut out = vec![];
    let mut offset = 0;
    for (i, raw) in bytes.split_inclusive(|b| *b == b'\n').enumerate() {
        if raw.len() > l.line_bytes {
            return Err(Error::new("LIMIT_LINE", "Line exceeds byte limit").at(i + 1));
        }
        let mut b = raw;
        if b.ends_with(b"\n") {
            b = &b[..b.len() - 1]
        }
        if b.ends_with(b"\r") {
            b = &b[..b.len() - 1]
        }
        if i == 0 && enc == Encoding::Utf8 && b.starts_with(b"\xef\xbb\xbf") {
            b = &b[3..]
        }
        let s = encoding::decode(b, enc).map_err(|e| e.at(i + 1))?;
        let s = s.trim_matches([' ', '\t']);
        if !s.is_empty() {
            if out.len() >= l.records {
                return Err(Error::new("LIMIT_RECORDS", "Too many records").at(i + 1));
            }
            let at = Location {
                line: i + 1,
                byte_start: offset,
                byte_end: offset + raw.len(),
                record: out.len(),
            };
            if s == "{" || s == "}" {
                out.push(Record {
                    tag: s.into(),
                    fields: vec![],
                    at,
                })
            } else {
                let t = tokens(s, l.fields).map_err(|e| e.at(i + 1))?;
                let Some(Field::Text(tag)) = t.first() else {
                    return Err(Error::new("LABEL", "Missing record label").at(i + 1));
                };
                if !tag.starts_with('#')
                    || tag.len() < 2
                    || !tag[1..]
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
                {
                    return Err(Error::new("LABEL", "Unsupported record label").at(i + 1));
                }
                out.push(Record {
                    tag: tag.clone(),
                    fields: t[1..].to_vec(),
                    at,
                });
            }
        }
        offset += raw.len();
    }
    if out.is_empty() {
        return Err(Error::new("EMPTY_INPUT", "Input contains no records"));
    }
    Ok(out)
}
pub fn text(r: &Record, i: usize) -> Result<&str> {
    match r.fields.get(i) {
        Some(Field::Text(s)) => Ok(s),
        _ => Err(Error::new("FIELD_TYPE", "Expected scalar field").at(r.at.line)),
    }
}
fn opt(r: &Record, i: usize) -> Result<Option<String>> {
    if i >= r.fields.len() {
        return Ok(None);
    }
    let s = text(r, i)?;
    Ok(if s.is_empty() { None } else { Some(s.into()) })
}
fn count(r: &Record, min: usize, max: usize) -> Result<()> {
    if r.fields.len() < min || r.fields.len() > max {
        Err(Error::new("FIELD_COUNT", "Unsupported field count").at(r.at.line))
    } else {
        Ok(())
    }
}
pub fn numeric(s: &str) -> Result<String> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        Err(Error::new(
            "NUMERIC_ID",
            "Expected nonempty numeric identifier",
        ))
    } else {
        Ok(s.into())
    }
}
fn year(r: &Record, i: usize) -> Result<i32> {
    let s = text(r, i)?;
    let n = s
        .parse::<i32>()
        .map_err(|_| Error::new("YEAR_SYNTAX", "Invalid relative fiscal year").at(r.at.line))?;
    if n > 0 || s.starts_with('+') {
        return Err(Error::new("YEAR_RANGE", "Fiscal year must be zero or negative").at(r.at.line));
    }
    Ok(n)
}
fn assignments(r: &Record, i: usize) -> Result<Vec<Assignment>> {
    let Some(Field::Objects(fields)) = r.fields.get(i) else {
        return Err(Error::new("OBJECT_TYPE", "Expected object list").at(r.at.line));
    };
    if fields.len() % 2 != 0 {
        return Err(
            Error::new("OBJECT_PAIRS", "Object list requires dimension/value pairs").at(r.at.line),
        );
    }
    let mut out = vec![];
    let mut seen = BTreeSet::new();
    for p in fields.as_chunks::<2>().0 {
        numeric(&p[0]).map_err(|e| e.at(r.at.line))?;
        if p[1].is_empty() || !seen.insert(p[0].clone()) {
            return Err(
                Error::new("OBJECT_AMBIGUOUS", "Empty value or repeated dimension").at(r.at.line),
            );
        }
        out.push(Assignment {
            dimension: p[0].clone(),
            object: p[1].clone(),
        });
    }
    out.sort();
    Ok(out)
}
pub fn parse(bytes: &[u8], enc: Encoding, l: &Limits) -> Result<Document> {
    let records = lexical(bytes, enc, l)?;
    let checksum = check_crc(&records)?;
    let mut d = Document {
        sha256: sha256(bytes),
        byte_length: bytes.len(),
        encoding: enc,
        sie_type: 1,
        type_defaulted: true,
        organization: None,
        organization_parts: vec![],
        currency: "SEK".into(),
        currency_defaulted: true,
        name: None,
        extent_date: None,
        chart_family: "BAS95".into(),
        chart_family_defaulted: true,
        years: BTreeMap::new(),
        accounts: BTreeMap::new(),
        dimensions: BTreeMap::new(),
        objects: vec![],
        balances: vec![],
        vouchers: vec![],
        records: vec![],
        unsupported: vec![],
        checksum,
        snapshot: None,
    };
    let mut current: Option<Voucher> = None;
    let mut open = false;
    let mut singleton = BTreeSet::new();
    for r in &records {
        let tag = r.tag.as_str();
        if open {
            if tag != "{" {
                return Err(
                    Error::new("VOUCHER_BLOCK", "Voucher requires an opening block").at(r.at.line),
                );
            }
            open = false;
            continue;
        }
        if tag == "{" {
            return Err(
                Error::new("BLOCK_CONTEXT", "Unsupported extension or nested block").at(r.at.line),
            );
        }
        if tag == "}" {
            d.vouchers.push(current.take().ok_or_else(|| {
                Error::new("BLOCK_CONTEXT", "Closing brace outside voucher").at(r.at.line)
            })?);
            continue;
        }
        if matches!(tag, "#TRANS" | "#RTRANS" | "#BTRANS") {
            let v = current.as_mut().ok_or_else(|| {
                Error::new("ROW_CONTEXT", "Transaction outside voucher").at(r.at.line)
            })?;
            if v.rows.len() >= l.rows_per_voucher {
                return Err(Error::new("LIMIT_ROWS", "Too many voucher rows").at(r.at.line));
            }
            count(r, 3, 7)?;
            let dt = opt(r, 3)?;
            if let Some(dt) = &dt {
                date(dt).map_err(|e| e.at(r.at.line))?
            }
            v.rows.push(Row {
                kind: match tag {
                    "#TRANS" => RowKind::Final,
                    "#RTRANS" => RowKind::AddedHistory,
                    _ => RowKind::RemovedHistory,
                },
                account: numeric(text(r, 0)?)?,
                objects: assignments(r, 1)?,
                amount: Money::major(text(r, 2)?).map_err(|e| e.at(r.at.line))?,
                date: dt,
                text: opt(r, 4)?,
                quantity: quantity(opt(r, 5)?.as_deref().unwrap_or(""))
                    .map_err(|e| e.at(r.at.line))?,
                signature: opt(r, 6)?,
                at: r.at.clone(),
            });
            continue;
        }
        if current.is_some() {
            if known(tag) {
                return Err(
                    Error::new("RECORD_CONTEXT", "Header or control inside voucher").at(r.at.line),
                );
            }
            d.unsupported.push(r.at.clone());
            continue;
        }
        if matches!(
            tag,
            "#FLAGGA"
                | "#FORMAT"
                | "#SIETYP"
                | "#ORGNR"
                | "#VALUTA"
                | "#FNAMN"
                | "#GEN"
                | "#OMFATTN"
                | "#FNR"
                | "#KPTYP"
                | "#FTYP"
                | "#TAXAR"
                | "#ADRESS"
                | "#BKOD"
        ) && !singleton.insert(tag.to_owned())
        {
            return Err(Error::new("DUPLICATE_HEADER", "Repeated singleton header").at(r.at.line));
        }
        match tag {
            "#VER" => {
                count(r, 3, 6)?;
                if d.vouchers.len() >= l.vouchers {
                    return Err(Error::new("LIMIT_VOUCHERS", "Too many vouchers").at(r.at.line));
                }
                date(text(r, 2)?).map_err(|e| e.at(r.at.line))?;
                let reg = opt(r, 4)?;
                if let Some(reg) = &reg {
                    date(reg).map_err(|e| e.at(r.at.line))?
                }
                current = Some(Voucher {
                    series: text(r, 0)?.into(),
                    number: text(r, 1)?.into(),
                    date: text(r, 2)?.into(),
                    text: opt(r, 3)?,
                    registered_on: reg,
                    signature: opt(r, 5)?,
                    rows: vec![],
                    at: r.at.clone(),
                });
                open = true;
            }
            "#SIETYP" => {
                count(r, 1, 1)?;
                d.sie_type = text(r, 0)?
                    .parse::<u8>()
                    .ok()
                    .filter(|n| (1..=4).contains(n))
                    .ok_or_else(|| {
                        Error::new("SIE_TYPE", "Only types 1 through 4 are supported").at(r.at.line)
                    })?;
                d.type_defaulted = false;
            }
            "#FLAGGA" => {
                count(r, 1, 1)?;
                if !matches!(text(r, 0)?, "0" | "1") {
                    return Err(Error::new("FLAG", "Import flag must be zero or one").at(r.at.line));
                }
            }
            "#FORMAT" => {
                count(r, 1, 1)?;
                let _ = text(r, 0)?;
            }
            "#ORGNR" => {
                count(r, 1, 3)?;
                d.organization = opt(r, 0)?;
                d.organization_parts = (1..r.fields.len())
                    .map(|i| text(r, i).map(str::to_owned))
                    .collect::<Result<Vec<_>>>()?;
            }
            "#FNAMN" => {
                count(r, 1, 1)?;
                d.name = opt(r, 0)?;
            }
            "#VALUTA" => {
                count(r, 1, 1)?;
                let c = text(r, 0)?;
                if c.len() != 3 || !c.bytes().all(|b| b.is_ascii_uppercase()) {
                    return Err(Error::new(
                        "CURRENCY",
                        "Currency requires three uppercase ASCII letters",
                    )
                    .at(r.at.line));
                }
                d.currency = c.into();
                d.currency_defaulted = false;
            }
            "#GEN" => {
                count(r, 1, 2)?;
                date(text(r, 0)?).map_err(|e| e.at(r.at.line))?;
            }
            "#OMFATTN" => {
                count(r, 1, 1)?;
                date(text(r, 0)?).map_err(|e| e.at(r.at.line))?;
                d.extent_date = opt(r, 0)?;
            }
            "#RAR" => {
                count(r, 3, 3)?;
                let y = year(r, 0)?;
                let p = Period::new(text(r, 1)?, text(r, 2)?).map_err(|e| e.at(r.at.line))?;
                if d.years.insert(y, p).is_some() {
                    return Err(Error::new("DUPLICATE_YEAR", "Repeated fiscal year").at(r.at.line));
                }
            }
            "#KONTO" => {
                count(r, 2, 2)?;
                let n = numeric(text(r, 0)?)?;
                let a = Account {
                    number: n.clone(),
                    name: text(r, 1)?.into(),
                    kind: None,
                    unit: None,
                };
                if d.accounts.insert(n, a).is_some() {
                    return Err(
                        Error::new("DUPLICATE_ACCOUNT", "Repeated account declaration")
                            .at(r.at.line),
                    );
                }
            }
            "#KTYP" | "#ENHET" => {
                count(r, 2, 2)?;
                let a = d.accounts.get_mut(text(r, 0)?).ok_or_else(|| {
                    Error::new("ACCOUNT_ORDER", "Account metadata precedes declaration")
                        .at(r.at.line)
                })?;
                if tag == "#KTYP" {
                    let k = text(r, 1)?;
                    if !matches!(k, "T" | "S" | "K" | "I") || a.kind.is_some() {
                        return Err(
                            Error::new("ACCOUNT_KIND", "Invalid or repeated account type")
                                .at(r.at.line),
                        );
                    }
                    a.kind = Some(k.into());
                } else {
                    if a.unit.is_some() {
                        return Err(
                            Error::new("ACCOUNT_UNIT", "Repeated account unit").at(r.at.line)
                        );
                    }
                    a.unit = Some(text(r, 1)?.into());
                }
            }
            "#DIM" | "#UNDERDIM" => {
                count(
                    r,
                    if tag == "#DIM" { 2 } else { 3 },
                    if tag == "#DIM" { 2 } else { 3 },
                )?;
                let id = numeric(text(r, 0)?)?;
                let dim = Dimension {
                    id: id.clone(),
                    name: text(r, 1)?.into(),
                    parent: if tag == "#UNDERDIM" {
                        Some(numeric(text(r, 2)?)?)
                    } else {
                        None
                    },
                };
                if d.dimensions.insert(id, dim).is_some() {
                    return Err(Error::new(
                        "DUPLICATE_DIMENSION",
                        "Repeated dimension declaration",
                    )
                    .at(r.at.line));
                }
            }
            "#OBJEKT" => {
                count(r, 3, 3)?;
                d.objects.push(Object {
                    dimension: numeric(text(r, 0)?)?,
                    number: text(r, 1)?.into(),
                    name: text(r, 2)?.into(),
                });
            }
            "#IB" | "#UB" | "#RES" | "#OIB" | "#OUB" | "#PSALDO" | "#PBUDGET" => {
                let (kind, mi, ai, oi, vi) = match tag {
                    "#IB" => (BalanceKind::Opening, None, 1, None, 2),
                    "#UB" => (BalanceKind::Closing, None, 1, None, 2),
                    "#RES" => (BalanceKind::Result, None, 1, None, 2),
                    "#OIB" => (BalanceKind::ObjectOpening, None, 1, Some(2), 3),
                    "#OUB" => (BalanceKind::ObjectClosing, None, 1, Some(2), 3),
                    "#PSALDO" => (BalanceKind::PeriodMovement, Some(1), 2, Some(3), 4),
                    _ => (BalanceKind::Budget, Some(1), 2, Some(3), 4),
                };
                count(r, vi + 1, vi + 2)?;
                let mon = mi.map(|i| text(r, i).map(str::to_owned)).transpose()?;
                if let Some(mon) = &mon {
                    month(mon).map_err(|e| e.at(r.at.line))?
                }
                d.balances.push(Balance {
                    kind,
                    year: year(r, 0)?,
                    month: mon,
                    account: numeric(text(r, ai)?)?,
                    objects: oi
                        .map(|i| assignments(r, i))
                        .transpose()?
                        .unwrap_or_default(),
                    amount: Money::major(text(r, vi)?).map_err(|e| e.at(r.at.line))?,
                    quantity: quantity(opt(r, vi + 1)?.as_deref().unwrap_or(""))
                        .map_err(|e| e.at(r.at.line))?,
                    at: r.at.clone(),
                });
            }
            "#KPTYP" => {
                count(r, 1, 1)?;
                let raw = text(r, 0)?;
                d.chart_family = if raw.starts_with("BAS2") {
                    "EUBAS97".into()
                } else {
                    raw.into()
                };
                d.chart_family_defaulted = false;
            }
            "#PROGRAM" => {
                count(r, 2, 2)?;
            }
            "#PROSA" | "#FTYP" | "#FNR" | "#BKOD" => {
                count(r, 1, 1)?;
            }
            "#ADRESS" => {
                count(r, 4, 4)?;
            }
            "#TAXAR" => {
                count(r, 1, 1)?;
                let s = text(r, 0)?;
                if s.len() != 4 || !s.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(
                        Error::new("TAX_YEAR", "Expected four-digit tax year").at(r.at.line)
                    );
                }
            }
            "#SRU" => {
                count(r, 2, 2)?;
                numeric(text(r, 0)?)?;
                numeric(text(r, 1)?)?;
            }
            "#KSUMMA" => {}
            _ => d.unsupported.push(r.at.clone()),
        }
    }
    if current.is_some() || open {
        return Err(Error::new("TRUNCATED_VOUCHER", "Unclosed voucher block"));
    }
    d.records = records;
    crate::validate::ensure_bounded(&d, l)?;
    Ok(d)
}
pub fn known(t: &str) -> bool {
    matches!(
        t,
        "#VER"
            | "#TRANS"
            | "#RTRANS"
            | "#BTRANS"
            | "#SIETYP"
            | "#FLAGGA"
            | "#FORMAT"
            | "#ORGNR"
            | "#FNAMN"
            | "#VALUTA"
            | "#GEN"
            | "#OMFATTN"
            | "#RAR"
            | "#KONTO"
            | "#KTYP"
            | "#ENHET"
            | "#DIM"
            | "#UNDERDIM"
            | "#OBJEKT"
            | "#IB"
            | "#UB"
            | "#RES"
            | "#OIB"
            | "#OUB"
            | "#PSALDO"
            | "#PBUDGET"
            | "#PROGRAM"
            | "#PROSA"
            | "#KPTYP"
            | "#FTYP"
            | "#FNR"
            | "#BKOD"
            | "#ADRESS"
            | "#TAXAR"
            | "#SRU"
            | "#KSUMMA"
    )
}
/// Standard reflected CRC-32. This detects corruption, NOT authenticity.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut c = !0u32;
    for b in bytes {
        c ^= *b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xedb88320
            } else {
                c >> 1
            };
        }
    }
    !c
}
pub fn crc_payload(records: &[Record]) -> Result<Vec<u8>> {
    let mut out = vec![];
    for r in records {
        if r.tag == "{" || r.tag == "}" {
            continue;
        }
        out.extend(encoding::cp437_encode(&r.tag)?);
        for f in &r.fields {
            match f {
                Field::Text(s) => out.extend(encoding::cp437_encode(s)?),
                Field::Objects(a) => {
                    for s in a {
                        out.extend(encoding::cp437_encode(s)?);
                    }
                }
            }
        }
    }
    Ok(out)
}
pub fn check_crc(r: &[Record]) -> Result<Checksum> {
    let p: Vec<_> = r
        .iter()
        .enumerate()
        .filter(|(_, r)| r.tag == "#KSUMMA")
        .map(|(i, _)| i)
        .collect();
    if p.is_empty() {
        return Ok(Checksum::Absent);
    }
    if p.len() != 2
        || p[0] != 1
        || r[0].tag != "#FLAGGA"
        || p[1] != r.len() - 1
        || !r[p[0]].fields.is_empty()
    {
        return Err(Error::new(
            "CHECKSUM_FRAME",
            "Invalid or truncated checksum frame",
        ));
    }
    count(&r[p[1]], 1, 1)?;
    let s = text(&r[p[1]], 0)?;
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::new("CHECKSUM_VALUE", "Invalid checksum value"));
    }
    let expected = s
        .parse::<u32>()
        .map_err(|_| Error::new("CHECKSUM_VALUE", "Checksum exceeds u32"))?;
    let got = crc32(&crc_payload(&r[p[0] + 1..p[1]])?);
    if got != expected {
        return Err(Error::new(
            "CHECKSUM_MISMATCH",
            "SIE checksum differs from its payload",
        ));
    }
    Ok(Checksum::Verified { value: got })
}
