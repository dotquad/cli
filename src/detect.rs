//! Works out what a bare `dotquad <query>` holds, the same way as `detect()` in
//! `@dotquad/net` (https://github.com/dotquad/net), plus links.

use std::net::{Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Ipv4,
    Ipv6,
    Cidr,
    Asn,
    Domain,
    Link,
}

pub fn detect(query: &str) -> Option<Kind> {
    let v = query.trim();
    if v.is_empty() {
        return None;
    }
    let lower = v.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(Kind::Link);
    }
    if let Some((addr, bits)) = v.split_once('/') {
        let max = if addr.parse::<Ipv4Addr>().is_ok() {
            32
        } else if addr.parse::<Ipv6Addr>().is_ok() {
            128
        } else {
            0
        };
        let digits = !bits.is_empty() && bits.len() <= 3 && bits.bytes().all(|b| b.is_ascii_digit());
        if max > 0 && digits && bits.parse::<u32>().is_ok_and(|n| n <= max) {
            return Some(Kind::Cidr);
        }
        return None;
    }
    if v.parse::<Ipv4Addr>().is_ok() {
        return Some(Kind::Ipv4);
    }
    if v.contains(':') && v.parse::<Ipv6Addr>().is_ok() {
        return Some(Kind::Ipv6);
    }
    if is_asn(&lower) {
        return Some(Kind::Asn);
    }
    if is_domain(&lower) {
        return Some(Kind::Domain);
    }
    None
}

fn is_asn(v: &str) -> bool {
    v.strip_prefix("as").is_some_and(|n| (1..=10).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit()))
}

/// A hostname with at least two labels and a letters-only top-level domain.
fn is_domain(v: &str) -> bool {
    if v.is_empty() || v.len() > 253 {
        return false;
    }
    let labels: Vec<&str> = v.split('.').collect();
    let Some((tld, rest)) = labels.split_last() else {
        return false;
    };
    if rest.is_empty() || !(2..=63).contains(&tld.len()) || !tld.bytes().all(|b| b.is_ascii_lowercase()) {
        return false;
    }
    rest.iter().all(|l| {
        let b = l.as_bytes();
        (1..=63).contains(&b.len())
            && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
            && b[0] != b'-'
            && b[b.len() - 1] != b'-'
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_dotquad_net_cases() {
        let cases: &[(&str, Option<Kind>)] = &[
            ("", None),
            ("8.8.8.8", Some(Kind::Ipv4)),
            ("  1.1.1.1 ", Some(Kind::Ipv4)),
            ("256.1.1.1", None),
            ("2606:4700:4700::1111", Some(Kind::Ipv6)),
            ("::ffff:8.8.8.8", Some(Kind::Ipv6)),
            ("1::2::3", None),
            ("10.0.0.0/22", Some(Kind::Cidr)),
            ("10.0.0.0/33", None),
            ("2001:db8::/32", Some(Kind::Cidr)),
            ("AS13335", Some(Kind::Asn)),
            ("as15169", Some(Kind::Asn)),
            ("example.com", Some(Kind::Domain)),
            ("sub.example.co.uk", Some(Kind::Domain)),
            ("8.8", None),
        ];
        for (input, want) in cases {
            assert_eq!(detect(input), *want, "{input:?}");
        }
    }

    #[test]
    fn links_and_edge_cases() {
        assert_eq!(detect("https://example.com/a?b=c"), Some(Kind::Link));
        assert_eq!(detect("HTTP://example.com"), Some(Kind::Link));
        assert_eq!(detect("Example.COM"), Some(Kind::Domain));
        assert_eq!(detect("-bad.com"), None);
        assert_eq!(detect("localhost"), None);
        assert_eq!(detect("13335"), None);
        assert_eq!(detect("10.0.0.0/"), None);
    }
}
