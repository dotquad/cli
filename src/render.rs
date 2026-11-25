//! Each endpoint's JSON as text for a terminal. Fields are read loosely, so a field the API
//! adds or drops shows as "-" rather than breaking the command.

use serde_json::Value;

use crate::api::seg;
use crate::style::{self, Tone, bold, dim, flag, ok};

// ---------- reading JSON ----------

fn get<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |v, k| v.get(k)).filter(|v| !v.is_null())
}

fn s<'a>(v: &'a Value, path: &str) -> Option<&'a str> {
    get(v, path).and_then(Value::as_str).filter(|s| !s.is_empty())
}

fn n(v: &Value, path: &str) -> Option<i64> {
    get(v, path).and_then(Value::as_i64)
}

fn b(v: &Value, path: &str) -> Option<bool> {
    get(v, path).and_then(Value::as_bool)
}

fn arr<'a>(v: &'a Value, path: &str) -> &'a [Value] {
    get(v, path).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn strs(v: &Value, path: &str) -> Vec<String> {
    arr(v, path).iter().filter_map(|x| x.as_str().map(String::from)).collect()
}

/// A number, or a big IPv6 count the API sends as a decimal string.
fn count(v: &Value, path: &str) -> Option<String> {
    match get(v, path)? {
        Value::Number(x) => x.as_u64().map(thousands),
        Value::String(x) => Some(x.clone()),
        _ => None,
    }
}

/// `2027-08-13T04:00:00Z` → `2027-08-13`.
fn date(v: &Value, path: &str) -> Option<String> {
    s(v, path).map(|d| d.chars().take(10).collect())
}

pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// `TLSv1.3` → `TLS 1.3`, `TLSv1` → `TLS 1.0`.
fn tls_name(p: &str) -> String {
    match p.strip_prefix("TLSv") {
        Some("1") => "TLS 1.0".into(),
        Some(v) => format!("TLS {v}"),
        None => p.to_string(),
    }
}

// ---------- layout ----------

/// Rows of label, value and an optional dimmed note, with the columns aligned.
#[derive(Default)]
struct Table {
    rows: Vec<(String, Option<String>, Option<String>)>,
}

impl Table {
    fn row(&mut self, label: &str, value: Option<String>) -> &mut Self {
        self.rows.push((label.into(), value, None));
        self
    }

    /// A row only when there's a value.
    fn opt(&mut self, label: &str, value: Option<String>) -> &mut Self {
        if value.is_some() {
            self.row(label, value);
        }
        self
    }

    fn row_note(&mut self, label: &str, value: Option<String>, note: Option<String>) -> &mut Self {
        self.rows.push((label.into(), value, note));
        self
    }

    fn render(&self) -> String {
        let lw = self.rows.iter().map(|r| r.0.chars().count()).max().unwrap_or(0) + 2;
        let has_notes = self.rows.iter().any(|r| r.2.is_some());
        let vw = if has_notes {
            self.rows.iter().map(|r| visible_len(r.1.as_deref().unwrap_or("-"))).max().unwrap_or(0) + 2
        } else {
            0
        };
        let mut out = String::new();
        for (label, value, note) in &self.rows {
            let value = value.as_deref().unwrap_or("-");
            let mut line = format!("{}{}", dim(&pad(label, lw)), pad(value, vw));
            if let Some(note) = note {
                line.push_str(&dim(&format!("({note})")));
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }
}

/// Characters a terminal shows, leaving out color codes.
fn visible_len(s: &str) -> usize {
    let mut n = 0;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            n += 1;
        }
    }
    n
}

fn pad(s: &str, width: usize) -> String {
    let len = visible_len(s);
    if len >= width { s.to_string() } else { format!("{s}{}", " ".repeat(width - len)) }
}

/// A link to the matching page on the site, marked as coming from the CLI.
fn site_link(site: &str, path: &str) -> String {
    let sep = if path.contains('?') { '&' } else { '?' };
    format!("{}{path}{sep}ref=cli", site.trim_end_matches('/'))
}

fn footer(site: &str, path: &str) -> String {
    format!("\n{} {}\n", dim("On dotquad.app:"), site_link(site, path))
}

/// A column before each finding: its width, and how to name a finding's area.
type AreaColumn<'a> = (usize, &'a dyn Fn(&Value) -> String);

/// Problems first (fail, then warn, most points first), then notes. Passes are left out.
fn findings(list: &[Value], title_width: Option<AreaColumn>) -> String {
    let mut problems: Vec<&Value> =
        list.iter().filter(|f| matches!(s(f, "level"), Some("fail" | "warn"))).collect();
    problems.sort_by_key(|f| (s(f, "level") != Some("fail"), -n(f, "points").unwrap_or(0)));
    let notes: Vec<&Value> = list.iter().filter(|f| s(f, "level") == Some("info")).collect();
    let mut out = String::new();
    let area = |f: &Value| match &title_width {
        Some((w, name)) => dim(&pad(&name(f), *w)),
        None => String::new(),
    };
    if problems.is_empty() {
        out.push_str(&format!("\n{}\n", ok("Nothing to fix.")));
    } else {
        out.push_str(&format!("\n{}\n", bold(&format!("To fix ({})", problems.len()))));
        for f in problems {
            let mark = if s(f, "level") == Some("fail") { flag("!") } else { "-".into() };
            out.push_str(&format!("{mark} {}{}\n", area(f), s(f, "title").unwrap_or("-")));
            if let Some(d) = s(f, "detail") {
                out.push_str(&format!("  {}\n", dim(d)));
            }
        }
    }
    if !notes.is_empty() {
        out.push_str(&format!("\n{}\n", bold("Notes")));
        for f in notes {
            out.push_str(&format!("· {}{}\n", area(f), s(f, "title").unwrap_or("-")));
            if let Some(d) = s(f, "detail") {
                out.push_str(&format!("  {}\n", dim(d)));
            }
        }
    }
    out
}

fn graded_head(name: &str, v: &Value) -> String {
    match s(v, "grade") {
        Some(g) => format!(
            "{}: {} {}\n",
            bold(name),
            style::grade(g, 0),
            dim(&format!("({} out of 100)", n(v, "score").unwrap_or(0)))
        ),
        None => format!("{}: no grade\n", bold(name)),
    }
}

// ---------- endpoints ----------

fn source(v: &Value, group: &str) -> Option<String> {
    let src = get(v, &format!("sources.{group}"))?;
    let name = s(src, "name")?;
    Some(match s(src, "updated") {
        Some(u) => format!("{name}, {u}"),
        None => name.to_string(),
    })
}

/// `GET /v1/ip/{ip}`, `/v1/me` and the open `/json`.
pub fn lookup(v: &Value, site: &str) -> String {
    let place =
        [s(v, "location.city"), s(v, "location.region")].into_iter().flatten().collect::<Vec<_>>().join(", ");
    let coords = match (get(v, "location.latitude"), get(v, "location.longitude")) {
        (Some(la), Some(lo)) => Some(format!("{la}, {lo}")),
        _ => None,
    };
    let network = n(v, "network.asn").map(|asn| match s(v, "network.org") {
        Some(org) => format!("AS{asn} {org}"),
        None => format!("AS{asn}"),
    });
    let mut t = Table::default();
    t.row("IP", s(v, "ip").map(bold).or(Some("-".into())));
    t.row("Version", n(v, "version").map(|x| format!("IPv{x}")));
    t.row_note("Hostname", s(v, "hostname").map(String::from), source(v, "hostname"));
    t.row_note("Location", Some(place).filter(|p| !p.is_empty()), source(v, "location"));
    t.row_note("Country", s(v, "location.country").map(String::from), source(v, "country"));
    t.row_note("Coordinates", coords, source(v, "location"));
    t.row_note("Timezone", s(v, "location.timezone").map(String::from), source(v, "timezone"));
    t.row_note("Network", network, source(v, "network"));
    t.row_note("Prefix", s(v, "network.prefix").map(String::from), source(v, "network"));
    if let Some(abuse) = s(v, "network.abuse") {
        t.row_note("Abuse contact", Some(abuse.into()), source(v, "abuse"));
    }
    match get(v, "privacy") {
        Some(p) => {
            let found: Vec<&str> =
                [("vpn", "VPN"), ("proxy", "proxy"), ("tor", "Tor"), ("hosting", "hosting")]
                    .into_iter()
                    .filter(|(k, _)| b(p, k) == Some(true))
                    .map(|(_, label)| label)
                    .collect();
            let value = if found.is_empty() {
                ok("none found")
            } else {
                let mut x = flag(&found.join(", "));
                if let Some(provider) = s(p, "provider") {
                    x.push_str(&format!(" ({provider})"));
                }
                x
            };
            t.row_note("Privacy", Some(value), source(v, "privacy"));
        }
        None => {
            t.row("Privacy", Some(dim("Starter plan and up")));
        }
    }
    let ip = s(v, "ip").unwrap_or_default();
    format!(
        "{}\n{}\n{}{}",
        t.render(),
        dim("Location is approximate, usually to about city level."),
        dim(
            "IP data: IPinfo (CC BY-SA 4.0) and DB-IP (CC BY 4.0). Timezones: Timezone Boundary Builder (ODbL)."
        ),
        footer(site, &format!("/?ip={}", seg(ip)))
    )
}

/// `GET /v1/asn/{asn}`.
pub fn asn(v: &Value, site: &str, all: bool) -> String {
    const SHOWN: usize = 20;
    let num = n(v, "asn").unwrap_or(0);
    let mut t = Table::default();
    t.row("ASN", Some(bold(&format!("AS{num}"))));
    t.row("Owner", s(v, "org").map(String::from));
    t.row("Domain", s(v, "domain").map(String::from));
    t.row("Country", s(v, "country").map(String::from));
    let v4 = strs(v, "prefixes.ipv4");
    let v6 = strs(v, "prefixes.ipv6");
    t.row(
        "Prefixes",
        Some(format!("{} IPv4, {} IPv6", thousands(v4.len() as u64), thousands(v6.len() as u64))),
    );
    let mut out = t.render();
    for (label, list) in [("IPv4 prefixes", &v4), ("IPv6 prefixes", &v6)] {
        if list.is_empty() {
            continue;
        }
        out.push_str(&format!("\n{}\n", bold(label)));
        let take = if all { list.len() } else { SHOWN.min(list.len()) };
        for p in &list[..take] {
            out.push_str(&format!("  {p}\n"));
        }
        if take < list.len() {
            out.push_str(&dim(&format!(
                "  and {} more (--all lists them)\n",
                thousands((list.len() - take) as u64)
            )));
        }
    }
    out.push_str(&footer(site, &format!("/asn/{num}")));
    out
}

/// `GET /v1/dns/{query}`: records for a name, or reverse DNS for an IP.
pub fn dns(v: &Value, site: &str) -> String {
    let q = s(v, "query").unwrap_or_default();
    let mut out = String::new();
    if s(v, "type") == Some("reverse") {
        let names = strs(v, "hostnames");
        out.push_str(&format!("{} {}\n", bold(s(v, "ip").unwrap_or(q)), dim("reverse DNS (PTR record)")));
        if names.is_empty() {
            out.push_str("  No PTR record.\n");
        }
        for h in names {
            out.push_str(&format!("  {h}\n"));
        }
        match b(v, "forward_confirmed") {
            Some(true) => out.push_str(&format!("{}\n", ok("The hostname points back to this address."))),
            Some(false) => {
                out.push_str(&format!("{}\n", flag("The hostname doesn't point back to this address.")))
            }
            None => {}
        }
        out.push_str(&footer(site, &format!("/dns/?q={}", seg(q))));
        return out;
    }
    out.push_str(&format!("{}\n\n", bold(s(v, "name").unwrap_or(q))));
    let mut rows: Vec<(String, String, Option<String>)> = Vec::new();
    let ttl = |r: &Value| n(r, "ttl").map(|t| format!("TTL {t}"));
    for r in arr(v, "records.A") {
        rows.push(("A".into(), s(r, "address").unwrap_or("-").into(), ttl(r)));
    }
    for r in arr(v, "records.AAAA") {
        rows.push(("AAAA".into(), s(r, "address").unwrap_or("-").into(), ttl(r)));
    }
    for r in strs(v, "records.CNAME") {
        rows.push(("CNAME".into(), r, None));
    }
    for r in arr(v, "records.MX") {
        let value = match s(r, "exchange") {
            Some(ex) => format!("{} {ex}", n(r, "priority").unwrap_or(0)),
            None => "null MX: accepts no email".into(),
        };
        rows.push(("MX".into(), value, None));
    }
    for r in strs(v, "records.NS") {
        rows.push(("NS".into(), r, None));
    }
    for r in strs(v, "records.TXT") {
        rows.push(("TXT".into(), r, None));
    }
    for r in arr(v, "records.CAA") {
        let value = match r {
            Value::String(x) => x.clone(),
            _ => format!(
                "{} {} \"{}\"",
                n(r, "critical").or(n(r, "flags")).unwrap_or(0),
                s(r, "tag").unwrap_or("-"),
                s(r, "value").unwrap_or("")
            ),
        };
        rows.push(("CAA".into(), value, None));
    }
    if let Some(soa) = get(v, "records.SOA") {
        rows.push((
            "SOA".into(),
            format!("{} {}", s(soa, "nsname").unwrap_or("-"), s(soa, "hostmaster").unwrap_or("-")),
            n(soa, "serial").map(|x| format!("serial {x}")),
        ));
    }
    if rows.is_empty() {
        out.push_str("No records.\n");
    }
    let vw = rows.iter().map(|r| r.1.chars().count()).max().unwrap_or(0) + 2;
    for (kind, value, note) in rows {
        let line = format!(
            "{}{}{}",
            dim(&pad(&kind, 7)),
            pad(&value, vw),
            note.map(|x| dim(&x)).unwrap_or_default()
        );
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.push_str(&footer(site, &format!("/dns/?q={}", seg(q))));
    out
}

/// `GET /v1/whois/{query}`.
pub fn whois(v: &Value, site: &str) -> String {
    let q = s(v, "query").unwrap_or_default();
    let mut t = Table::default();
    t.row("Name", s(v, "name").or(s(v, "object")).map(bold));
    if let Some(net) = get(v, "network").filter(|x| x.is_object()) {
        let range = s(net, "range").or(s(net, "cidr")).or(s(net, "start")).map(String::from);
        t.opt("Network", range);
        if let Some(name) = s(net, "name") {
            t.row("Network name", Some(name.into()));
        }
    }
    if let Some(h) = get(v, "holder") {
        let holder = match h {
            Value::String(x) => Some(x.clone()),
            _ => s(h, "name").or(s(h, "organization")).map(String::from),
        };
        t.opt("Holder", holder);
    }
    t.opt("Registrar", s(v, "registrar.name").map(String::from));
    t.opt("Registered", date(v, "dates.registered"));
    t.opt("Updated", date(v, "dates.updated"));
    t.opt("Expires", date(v, "dates.expires"));
    let status = strs(v, "status");
    if !status.is_empty() {
        t.row("Status", Some(status.join(", ")));
    }
    let ns = strs(v, "nameservers");
    if !ns.is_empty() {
        t.row("Nameservers", Some(ns.join(", ")));
    }
    if get(v, "dnssec").is_some() {
        t.row("DNSSEC", b(v, "dnssec").map(|x| if x { "signed" } else { "unsigned" }.to_string()));
    }
    if let Some(a) = s(v, "abuse_email") {
        t.row("Abuse contact", Some(a.into()));
    }
    let src = match (s(v, "source.protocol"), s(v, "source.server")) {
        (Some(p), Some(srv)) => Some(format!("{} from {srv}", p.to_uppercase())),
        (Some(p), None) => Some(p.to_uppercase()),
        _ => None,
    };
    t.opt("Source", src);
    let mut out = t.render();
    let contacts = arr(v, "contacts");
    if !contacts.is_empty() {
        out.push_str(&format!("\n{}\n", bold("Contacts")));
        let rw = contacts.iter().map(|c| strs(c, "roles").join(", ").chars().count()).max().unwrap_or(0) + 2;
        for c in contacts {
            let roles = strs(c, "roles").join(", ");
            let who =
                [s(c, "name"), s(c, "organization")].into_iter().flatten().collect::<Vec<_>>().join(", ");
            let emails = strs(c, "emails").join(", ");
            let mut line = format!("  {}{}", dim(&pad(&roles, rw)), if who.is_empty() { "-" } else { &who });
            if !emails.is_empty() {
                line.push_str(&format!("  {emails}"));
            }
            out.push_str(&line);
            out.push('\n');
        }
    }
    if let Some(raw) = s(v, "raw") {
        out.push_str(&format!("\n{}\n{}\n", bold("Registry record"), raw.trim_end()));
    }
    out.push_str(&footer(site, &format!("/whois/?q={}", seg(q))));
    out
}

fn dnssec_tone(status: &str) -> Tone {
    match status {
        "secure" => Tone::Ok,
        "bogus" => Tone::Flag,
        _ => Tone::Neutral,
    }
}

/// `GET /v1/dnssec/{name}`.
pub fn dnssec(v: &Value, site: &str) -> String {
    let name = s(v, "name").or(s(v, "query")).unwrap_or_default();
    let status = s(v, "status").unwrap_or("-");
    let mut out = format!("{}: {}\n", bold(name), bold(&style::tone(dnssec_tone(status), status)));
    if let Some(sum) = s(v, "summary") {
        out.push_str(&format!("{sum}\n"));
    }
    let zones = arr(v, "zones");
    if !zones.is_empty() {
        out.push_str(&format!("\n{}\n", bold("Chain of trust")));
        let zw = zones.iter().map(|z| s(z, "zone").unwrap_or("-").chars().count()).max().unwrap_or(0) + 2;
        for z in zones {
            let st = s(z, "status").unwrap_or("-");
            let keys = arr(z, "keys").len();
            let ds = arr(z, "ds").len();
            let mut line = format!(
                "  {}{}{}",
                pad(s(z, "zone").unwrap_or("-"), zw),
                style::tone(dnssec_tone(st), &pad(st, 11)),
                dim(&format!("{}, {ds} DS", plural(keys, "key")))
            );
            if let Some(p) = s(z, "problem") {
                line.push_str(&format!("\n    {}", flag(p)));
            }
            out.push_str(&line);
            out.push('\n');
        }
    }
    if let Some(r) = get(v, "record") {
        let st = s(r, "status").unwrap_or("-");
        out.push_str(&format!(
            "  {} record {}\n",
            s(r, "type").unwrap_or("-"),
            style::tone(dnssec_tone(st), st)
        ));
        if let Some(p) = s(r, "problem") {
            out.push_str(&format!("    {}\n", flag(p)));
        }
    }
    for w in strs(v, "warnings") {
        out.push_str(&format!("{} {w}\n", flag("-")));
    }
    out.push_str(&footer(site, &format!("/dns/?q={}", seg(name))));
    out
}

/// `GET /v1/tls/{host}`.
pub fn tls(v: &Value, site: &str) -> String {
    let host = s(v, "host").or(s(v, "query")).unwrap_or_default();
    let mut out = graded_head(host, v);
    out.push('\n');
    let mut t = Table::default();
    if let Some(c) = get(v, "certificate") {
        let trusted = match b(c, "trusted") {
            Some(true) => ok("trusted"),
            Some(false) => flag(s(c, "trust_error").unwrap_or("not trusted")),
            None => "-".into(),
        };
        t.row("Certificate", Some(format!("{} ({trusted})", s(c, "subject").unwrap_or("-"))));
        t.row("Issuer", s(c, "issuer_org").or(s(c, "issuer")).map(String::from));
        let left = n(c, "days_left").map(|d| {
            let text = format!("{} ({d} days left)", date(c, "valid_to").unwrap_or_default());
            if d < 14 { flag(&text) } else { text }
        });
        t.row("Expires", left);
        let key = match (s(c, "key.type"), n(c, "key.bits")) {
            (Some(k), Some(bits)) => Some(format!("{} {bits}", k.to_uppercase())),
            (Some(k), None) => Some(k.to_uppercase()),
            _ => None,
        };
        t.row("Key", key);
        if b(c, "hostname_match") == Some(false) {
            t.row("Names", Some(flag("The certificate doesn't cover this host")));
        }
    }
    if let Some(p) = get(v, "protocols").and_then(Value::as_object) {
        let on: Vec<String> =
            p.iter().filter(|(_, x)| x.as_bool() == Some(true)).map(|(k, _)| tls_name(k)).collect();
        t.row("Accepts", Some(on.join(", ")).filter(|x| !x.is_empty()));
    }
    let conn = [s(v, "connection.protocol").map(tls_name), s(v, "connection.cipher").map(String::from)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");
    t.row("Connection", Some(conn).filter(|x| !x.is_empty()));
    t.row(
        "HTTP to HTTPS",
        b(v, "http.redirects_to_https").map(|x| if x { ok("redirects") } else { flag("doesn't redirect") }),
    );
    out.push_str(&t.render());
    out.push_str(&findings(arr(v, "findings"), None));
    out.push_str(&footer(site, &format!("/tls/?q={}", seg(host))));
    out
}

/// `GET /v1/email/{domain}`.
pub fn email(v: &Value, site: &str) -> String {
    let domain = s(v, "domain").or(s(v, "query")).unwrap_or_default();
    let mut out = graded_head(domain, v);
    if let Some(sp) = s(v, "spoofing") {
        let text = match sp {
            "protected" => ok("Protected from spoofing: DMARC quarantines or rejects failing mail"),
            "unknown" => "Spoofing protection unknown".into(),
            _ => flag("Not protected from spoofing"),
        };
        out.push_str(&format!("{text}\n"));
    }
    out.push('\n');
    let mut t = Table::default();
    let mx = arr(v, "mx.hosts").iter().filter_map(|h| s(h, "exchange")).collect::<Vec<_>>().join(", ");
    t.row(
        "MX",
        if s(v, "mx.status") == Some("null") {
            Some("null MX: accepts no email".into())
        } else {
            Some(mx).filter(|x| !x.is_empty())
        },
    );
    let spf = s(v, "spf.record").map(|r| match (n(v, "spf.lookups"), n(v, "spf.limit")) {
        (Some(l), Some(lim)) => format!("{r} {}", dim(&format!("({l} of {lim} lookups)"))),
        _ => r.to_string(),
    });
    t.row("SPF", spf.or(s(v, "spf.status").map(String::from)));
    let dmarc = s(v, "dmarc.record").map(String::from).or(s(v, "dmarc.status").map(String::from));
    t.row("DMARC", dmarc);
    let keys = arr(v, "dkim.keys").len();
    let dkim = match s(v, "dkim.status") {
        _ if keys > 0 => Some(format!("{keys} key{}", if keys == 1 { "" } else { "s" })),
        Some(st) => Some(st.to_string()),
        None => None,
    };
    t.row("DKIM", dkim);
    t.row("MTA-STS", s(v, "mta_sts.status").map(String::from));
    t.row("TLS-RPT", s(v, "tls_rpt.status").map(String::from));
    t.row("BIMI", s(v, "bimi.status").map(String::from));
    out.push_str(&t.render());
    let area = |f: &Value| s(f, "area").unwrap_or("").replace('_', "-").to_uppercase();
    out.push_str(&findings(arr(v, "findings"), Some((10, &area))));
    out.push_str(&footer(site, &format!("/email/?q={}", seg(domain))));
    out
}

/// `GET /v1/check/{domain}`: the domain report card.
pub fn check(v: &Value, site: &str) -> String {
    const MAX_PROBLEMS: usize = 12;
    let domain = s(v, "domain").or(s(v, "query")).unwrap_or_default();
    let mut out = graded_head(domain, v);
    out.push('\n');
    let sections = arr(v, "sections");
    let tw = sections.iter().map(|x| s(x, "title").unwrap_or("").chars().count()).max().unwrap_or(0) + 2;
    for sec in sections {
        let g = if s(sec, "status") == Some("ok") { s(sec, "grade").unwrap_or("-") } else { "-" };
        let line = format!(
            "{}{}{}",
            if g == "-" { pad("-", 5) } else { style::grade(g, 5) },
            pad(s(sec, "title").unwrap_or("-"), tw),
            s(sec, "summary").unwrap_or("")
        );
        out.push_str(line.trim_end());
        out.push('\n');
    }
    let mut problems: Vec<(&Value, &Value)> = sections
        .iter()
        .flat_map(|sec| arr(sec, "findings").iter().map(move |f| (sec, f)))
        .filter(|(_, f)| matches!(s(f, "level"), Some("fail" | "warn")))
        .collect();
    problems.sort_by_key(|(_, f)| (s(f, "level") != Some("fail"), -n(f, "points").unwrap_or(0)));
    if problems.is_empty() {
        out.push_str(&format!("\n{}\n", ok("Nothing to fix.")));
    } else {
        out.push_str(&format!("\n{}\n", bold(&format!("To fix ({})", problems.len()))));
        for (sec, f) in problems.iter().take(MAX_PROBLEMS) {
            let mark = if s(f, "level") == Some("fail") { flag("!") } else { "-".into() };
            out.push_str(&format!(
                "{mark} {}{}\n",
                dim(&pad(s(sec, "title").unwrap_or("-"), tw)),
                s(f, "title").unwrap_or("-")
            ));
        }
        if problems.len() > MAX_PROBLEMS {
            out.push_str(&dim(&format!("  and {} more\n", problems.len() - MAX_PROBLEMS)));
        }
    }
    out.push_str(&format!(
        "\n{} {}\n",
        dim("Full report, with how to fix each:"),
        site_link(site, &format!("/check/{}", seg(domain)))
    ));
    out
}

fn takeover_tone(status: &str) -> Tone {
    match status {
        "takeover" | "dangling" => Tone::Flag,
        "ok" => Tone::Ok,
        _ => Tone::Neutral,
    }
}

/// `GET /v1/takeover/{domain}`.
pub fn takeover(v: &Value, site: &str) -> String {
    let domain = s(v, "domain").or(s(v, "query")).unwrap_or_default();
    let take = n(v, "takeover").unwrap_or(0);
    let dangling = n(v, "dangling").unwrap_or(0);
    let mut out = format!(
        "{}: {} names checked, {} open to takeover, {} dangling\n",
        bold(domain),
        n(v, "checked").unwrap_or(0),
        if take > 0 { flag(&take.to_string()) } else { take.to_string() },
        if dangling > 0 { flag(&dangling.to_string()) } else { dangling.to_string() },
    );
    let names = arr(v, "names");
    if names.is_empty() {
        out.push_str("No subdomains found beyond the domain itself.\n");
    } else {
        out.push('\n');
        let nw = names.iter().map(|x| s(x, "name").unwrap_or("").chars().count()).max().unwrap_or(0) + 2;
        for x in names {
            let st = s(x, "status").unwrap_or("-");
            out.push_str(&format!(
                "{}{}{}\n",
                style::tone(takeover_tone(st), &pad(st, 10)),
                pad(s(x, "name").unwrap_or("-"), nw),
                s(x, "title").unwrap_or("")
            ));
            let cname = strs(x, "cname");
            if !cname.is_empty() {
                out.push_str(&format!("{}\n", dim(&format!("          → {}", cname.join(" → ")))));
            }
            if matches!(st, "takeover" | "dangling" | "check")
                && let Some(d) = s(x, "detail")
            {
                out.push_str(&format!("          {}\n", dim(d)));
            }
        }
    }
    out.push_str(&dim("\nDNS only. Certificate logs aren't searched.\n"));
    out.push_str(&footer(site, &format!("/takeover/?q={}", seg(domain))));
    out
}

/// `GET /v1/link?url=`.
pub fn link(v: &Value, site: &str) -> String {
    let url = s(v, "url").or(s(v, "query")).unwrap_or_default();
    let verdict = match s(v, "verdict") {
        Some("warning_signs") => flag("Warning signs"),
        Some("caution") => flag("Caution"),
        Some("no_warning_signs") => ok("No warning signs"),
        Some(other) => other.to_string(),
        None => "-".into(),
    };
    let mut out = format!("{}\n{}\n", bold(url), bold(&verdict));
    let redirects = arr(v, "redirects").len();
    if let Some(fin) = s(v, "final.url").filter(|f| *f != url) {
        out.push_str(&format!(
            "Leads to {fin} {}\n",
            dim(&format!("after {redirects} redirect{}", if redirects == 1 { "" } else { "s" }))
        ));
    }
    if let Some(stop) = s(v, "stopped") {
        out.push_str(&format!("{}\n", flag(stop)));
    }
    out.push('\n');
    for sig in arr(v, "signals") {
        let mark = match s(sig, "level") {
            Some("danger") => flag("!"),
            Some("warning") => flag("-"),
            Some("good") => ok("✓"),
            _ => "·".into(),
        };
        out.push_str(&format!("{mark} {}\n", s(sig, "title").unwrap_or("-")));
        if let Some(d) = s(sig, "detail") {
            out.push_str(&format!("  {}\n", dim(d)));
        }
    }
    out.push_str(&dim("\nNot a phishing or malware list. It points out warning signs.\n"));
    out.push_str(&footer(site, &format!("/link-check/?q={}", seg(url))));
    out
}

/// `GET /v1/blacklist/{query}`.
pub fn blacklist(v: &Value, site: &str) -> String {
    let q = s(v, "query").unwrap_or_default();
    let listed = b(v, "listed") == Some(true);
    let mut out = format!("{}: {}\n", bold(q), if listed { flag("listed") } else { ok("not listed") });
    if let Some(note) = s(v, "note") {
        out.push_str(&format!("{note}\n"));
    }
    let names: std::collections::HashMap<&str, &str> =
        arr(v, "lists").iter().filter_map(|l| Some((s(l, "id")?, s(l, "name")?))).collect();
    for a in arr(v, "addresses") {
        let role = match (s(a, "role"), s(a, "host")) {
            (Some("mx"), Some(h)) => format!("mail server {h}"),
            (Some("a"), _) => "A record".into(),
            _ => String::new(),
        };
        out.push_str(&format!("\n{} {}\n", bold(s(a, "ip").unwrap_or("-")), dim(&role)));
        for r in arr(a, "results") {
            let id = s(r, "list").unwrap_or("-");
            let name = names.get(id).copied().unwrap_or(id);
            let st = s(r, "status").unwrap_or("-");
            let text = match st {
                "listed" => flag("listed"),
                "clean" => ok("clean"),
                "unsupported" => dim("doesn't cover IPv6"),
                "error" => dim("didn't answer"),
                x => x.to_string(),
            };
            out.push_str(&format!("  {}{text}\n", pad(name, 10)));
            if st == "listed" {
                if let Some(reason) = s(r, "reason") {
                    out.push_str(&format!("    {}\n", dim(reason)));
                }
                if let Some(u) = s(r, "removal_url") {
                    out.push_str(&format!("    {} {u}\n", dim("Removal:")));
                }
            }
        }
    }
    out.push_str(&footer(site, &format!("/blacklist/?q={}", seg(q))));
    out
}

/// `GET /v1/subnet/{cidr}`.
pub fn subnet(v: &Value, site: &str) -> String {
    let cidr = s(v, "cidr").unwrap_or_default();
    let mut t = Table::default();
    t.row("Block", Some(bold(cidr)));
    t.row("Kind", s(v, "kind").map(String::from));
    t.row("Network", s(v, "network").map(String::from));
    t.row("Netmask", s(v, "netmask").map(String::from));
    if get(v, "wildcard").is_some() {
        t.row("Wildcard", s(v, "wildcard").map(String::from));
        t.row("Broadcast", s(v, "broadcast").map(String::from));
    }
    t.row("First host", s(v, "first_host").map(String::from));
    t.row("Last host", s(v, "last_host").map(String::from));
    t.row("Addresses", count(v, "total_addresses"));
    t.row("Usable hosts", count(v, "usable_hosts"));
    let mut out = t.render();
    if let Some(split) = get(v, "split") {
        let subnets = strs(split, "subnets");
        out.push_str(&format!(
            "\n{}\n",
            bold(&format!(
                "Split into /{}: {}",
                n(split, "prefix").unwrap_or(0),
                count(split, "count").unwrap_or_default()
            ))
        ));
        for x in &subnets {
            out.push_str(&format!("  {x}\n"));
        }
        if b(split, "truncated") == Some(true) {
            out.push_str(&dim(&format!("  The first {} are listed.\n", subnets.len())));
        }
    }
    out.push_str(&footer(site, &format!("/subnet/?q={}", seg(cidr))));
    out
}

/// `GET /v1/usage`.
pub fn usage(v: &Value, site: &str) -> String {
    let used = n(v, "requests_used").unwrap_or(0).max(0) as u64;
    let quota = n(v, "requests_quota").unwrap_or(0).max(0) as u64;
    let left = n(v, "requests_remaining").unwrap_or(0).max(0) as u64;
    let plan = s(v, "plan").map(|p| {
        let mut c = p.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    });
    let mut t = Table::default();
    t.row("Plan", plan.map(|p| bold(&p)));
    let requests = format!("{} of {} this month", thousands(used), thousands(quota));
    let near = quota > 0 && used * 10 >= quota * 8;
    t.row_note(
        "Requests",
        Some(if near { flag(&requests) } else { requests }),
        Some(format!("{} left", thousands(left))),
    );
    t.row("Resets", date(v, "resets_at"));
    t.row("Rate limit", n(v, "requests_per_second").map(|r| format!("{r} requests a second")));
    format!("{}{}", t.render(), footer(site, "/dashboard/"))
}
