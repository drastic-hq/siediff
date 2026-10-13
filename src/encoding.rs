use crate::{model::Encoding, Error, Result};
pub const CP437: [char; 128] = [
    '\u{c7}', '\u{fc}', '\u{e9}', '\u{e2}', '\u{e4}', '\u{e0}', '\u{e5}', '\u{e7}', '\u{ea}',
    '\u{eb}', '\u{e8}', '\u{ef}', '\u{ee}', '\u{ec}', '\u{c4}', '\u{c5}', '\u{c9}', '\u{e6}',
    '\u{c6}', '\u{f4}', '\u{f6}', '\u{f2}', '\u{fb}', '\u{f9}', '\u{ff}', '\u{d6}', '\u{dc}',
    '\u{a2}', '\u{a3}', '\u{a5}', '\u{20a7}', '\u{192}', '\u{e1}', '\u{ed}', '\u{f3}', '\u{fa}',
    '\u{f1}', '\u{d1}', '\u{aa}', '\u{ba}', '\u{bf}', '\u{2310}', '\u{ac}', '\u{bd}', '\u{bc}',
    '\u{a1}', '\u{ab}', '\u{bb}', '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}',
    '\u{2561}', '\u{2562}', '\u{2556}', '\u{2555}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255d}',
    '\u{255c}', '\u{255b}', '\u{2510}', '\u{2514}', '\u{2534}', '\u{252c}', '\u{251c}', '\u{2500}',
    '\u{253c}', '\u{255e}', '\u{255f}', '\u{255a}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}',
    '\u{2550}', '\u{256c}', '\u{2567}', '\u{2568}', '\u{2564}', '\u{2565}', '\u{2559}', '\u{2558}',
    '\u{2552}', '\u{2553}', '\u{256b}', '\u{256a}', '\u{2518}', '\u{250c}', '\u{2588}', '\u{2584}',
    '\u{258c}', '\u{2590}', '\u{2580}', '\u{3b1}', '\u{df}', '\u{393}', '\u{3c0}', '\u{3a3}',
    '\u{3c3}', '\u{b5}', '\u{3c4}', '\u{3a6}', '\u{398}', '\u{3a9}', '\u{3b4}', '\u{221e}',
    '\u{3c6}', '\u{3b5}', '\u{2229}', '\u{2261}', '\u{b1}', '\u{2265}', '\u{2264}', '\u{2320}',
    '\u{2321}', '\u{f7}', '\u{2248}', '\u{b0}', '\u{2219}', '\u{b7}', '\u{221a}', '\u{207f}',
    '\u{b2}', '\u{25a0}', '\u{a0}',
];
const CP1252: [u32; 32] = [
    8364, 0, 8218, 402, 8222, 8230, 8224, 8225, 710, 8240, 352, 8249, 338, 0, 381, 0, 0, 8216,
    8217, 8220, 8221, 8226, 8211, 8212, 732, 8482, 353, 8250, 339, 0, 382, 376,
];
pub fn decode(b: &[u8], e: Encoding) -> Result<String> {
    match e {
        Encoding::Utf8 => std::str::from_utf8(b)
            .map(str::to_owned)
            .map_err(|_| Error::new("ENCODING", "Invalid UTF-8; lossy decoding disabled")),
        Encoding::Cp437 => Ok(b
            .iter()
            .map(|b| {
                if *b < 128 {
                    *b as char
                } else {
                    CP437[(*b - 128) as usize]
                }
            })
            .collect()),
        Encoding::Windows1252 => {
            let mut s = String::new();
            for b in b {
                let n = if (128..160).contains(b) {
                    CP1252[(*b - 128) as usize]
                } else {
                    *b as u32
                };
                if n == 0 && *b != 0 {
                    return Err(Error::new("ENCODING", "Undefined Windows-1252 character"));
                }
                s.push(char::from_u32(n).ok_or_else(|| Error::new("ENCODING", "Invalid scalar"))?)
            }
            Ok(s)
        }
    }
}
pub fn cp437_encode(s: &str) -> Result<Vec<u8>> {
    s.chars()
        .map(|c| {
            if (c as u32) < 128 {
                Ok(c as u8)
            } else {
                CP437
                    .iter()
                    .position(|x| *x == c)
                    .map(|i| (i + 128) as u8)
                    .ok_or_else(|| {
                        Error::new("CHECKSUM_ENCODING", "Field is not CP437-representable")
                    })
            }
        })
        .collect()
}
