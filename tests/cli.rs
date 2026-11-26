//! Runs the built binary against a mock API. Nothing here calls the real Dotquad API.

use assert_cmd::Command;
use httpmock::prelude::*;
use predicates::prelude::*;
use serde_json::json;
use tempfile::TempDir;

const KEY: &str = "dq_live_test0000000000000000abcd";

struct Env {
    server: MockServer,
    dir: TempDir,
}

impl Env {
    fn new() -> Self {
        Env { server: MockServer::start(), dir: tempfile::tempdir().unwrap() }
    }

    /// The binary with no key, a private config file and the mock API.
    fn cmd(&self) -> Command {
        let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("dotquad"));
        cmd.env_clear()
            .env("DOTQUAD_API_URL", self.server.base_url())
            .env("DOTQUAD_SITE_URL", "https://dotquad.test")
            .env("DOTQUAD_CONFIG", self.dir.path().join("config.toml"))
            .env("NO_COLOR", "1");
        // Windows can't open a socket without SystemRoot (os error 10106).
        if let Some(root) = std::env::var_os("SystemRoot") {
            cmd.env("SystemRoot", root);
        }
        cmd
    }

    fn keyed(&self) -> Command {
        let mut cmd = self.cmd();
        cmd.env("DOTQUAD_KEY", KEY);
        cmd
    }
}

fn lookup(ip: &str) -> serde_json::Value {
    json!({
        "ip": ip, "version": 4, "hostname": "one.one.one.one",
        "location": { "city": "Sydney", "region": "New South Wales", "country": "AU",
            "latitude": -33.8688, "longitude": 151.209, "timezone": "Australia/Sydney" },
        "network": { "asn": 13335, "org": "Cloudflare, Inc.", "prefix": "1.1.1.0/24", "type": "isp" },
        "privacy": null,
        "sources": { "network": { "name": "IPinfo Lite", "license": "CC BY-SA 4.0", "updated": "2026-11-25" } }
    })
}

#[test]
fn ip_lookup_sends_the_key_and_prints_a_table() {
    let env = Env::new();
    let m = env.server.mock(|when, then| {
        when.method(GET).path("/v1/ip/1.1.1.1").header("authorization", format!("Bearer {KEY}"));
        then.status(200).json_body(lookup("1.1.1.1"));
    });
    env.keyed()
        .arg("1.1.1.1")
        .assert()
        .success()
        .stdout(predicate::str::contains("AS13335 Cloudflare, Inc.  (IPinfo Lite, 2026-11-25)"))
        .stdout(predicate::str::contains("Sydney, New South Wales"))
        .stdout(predicate::str::contains("Starter plan and up"))
        .stdout(predicate::str::contains("https://dotquad.test/?ip=1.1.1.1&ref=cli"));
    m.assert();
}

#[test]
fn json_prints_the_api_answer() {
    let env = Env::new();
    env.server.mock(|when, then| {
        when.method(GET).path("/v1/ip/1.1.1.1");
        then.status(200).json_body(lookup("1.1.1.1"));
    });
    let out = env.keyed().args(["ip", "1.1.1.1", "--json"]).assert().success().get_output().stdout.clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v, lookup("1.1.1.1"));
}

#[test]
fn without_a_key_your_ip_uses_the_open_endpoint() {
    let env = Env::new();
    let m = env.server.mock(|when, then| {
        when.method(GET).path("/json").header_missing("authorization");
        then.status(200).json_body(lookup("203.0.113.7"));
    });
    env.cmd().assert().success().stdout(predicate::str::contains("203.0.113.7"));
    m.assert();
}

#[test]
fn without_a_key_a_domain_gets_the_open_report_card() {
    let env = Env::new();
    let m = env.server.mock(|when, then| {
        when.method(GET).path("/check/example.com/json");
        then.status(200).json_body(json!({
            "domain": "example.com", "grade": "C", "score": 66,
            "sections": [
                { "title": "HTTPS and headers", "status": "ok", "grade": "F", "summary": "no HSTS",
                  "findings": [{ "level": "fail", "title": "No HSTS", "points": 15 }] },
                { "title": "Email", "status": "ok", "grade": "A", "summary": "SPF -all", "findings": [] }
            ]
        }));
    });
    env.cmd()
        .arg("example.com")
        .assert()
        .success()
        .stdout(predicate::str::contains("example.com: C (66 out of 100)"))
        .stdout(predicate::str::contains("! HTTPS and headers  No HSTS"))
        .stdout(predicate::str::contains("https://dotquad.test/check/example.com?ref=cli"));
    m.assert();
}

#[test]
fn keyed_commands_explain_how_to_get_a_key() {
    let env = Env::new();
    env.cmd()
        .args(["dns", "example.com"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("`dotquad dns` needs an API key."))
        .stderr(predicate::str::contains("https://dotquad.test/signup/?ref=cli"));
}

#[test]
fn bare_queries_are_routed_by_kind() {
    let env = Env::new();
    let asn = env.server.mock(|when, then| {
        when.method(GET).path("/v1/asn/AS13335");
        then.status(200).json_body(json!({ "asn": 13335, "org": "Cloudflare, Inc.",
            "prefixes": { "ipv4": ["1.1.1.0/24"], "ipv6": [] } }));
    });
    let subnet = env.server.mock(|when, then| {
        when.method(GET).path("/v1/subnet/10.0.0.0%2F22");
        then.status(200).json_body(json!({ "cidr": "10.0.0.0/22", "kind": "Private (RFC 1918)",
            "total_addresses": 1024, "usable_hosts": 1022, "wildcard": "0.0.3.255", "broadcast": "10.0.3.255" }));
    });
    let link = env.server.mock(|when, then| {
        when.method(GET).path("/v1/link").query_param("url", "https://example.com/a?b=c");
        then.status(200)
            .json_body(json!({ "url": "https://example.com/a?b=c", "verdict": "caution", "signals": [] }));
    });
    env.keyed().arg("AS13335").assert().success().stdout(predicate::str::contains("1 IPv4, 0 IPv6"));
    env.keyed().arg("10.0.0.0/22").assert().success().stdout(predicate::str::contains("1,022"));
    env.keyed()
        .arg("https://example.com/a?b=c")
        .assert()
        .success()
        .stdout(predicate::str::contains("Caution"));
    env.keyed().arg("not a thing").assert().failure().stderr(predicate::str::contains("Not an IP address"));
    asn.assert();
    subnet.assert();
    link.assert();
}

#[test]
fn options_become_query_parameters() {
    let env = Env::new();
    let m = env.server.mock(|when, then| {
        when.method(GET).path("/v1/takeover/example.com").query_param("names", "www,api");
        then.status(200).json_body(json!({ "domain": "example.com", "checked": 52, "names": [] }));
    });
    let split = env.server.mock(|when, then| {
        when.method(GET).path("/v1/subnet/192.168.1.10%20255.255.255.0").query_param("split_prefix", "26");
        then.status(200).json_body(json!({ "cidr": "192.168.1.0/24",
            "split": { "prefix": 26, "count": 4, "subnets": ["192.168.1.0/26"], "truncated": false } }));
    });
    env.keyed().args(["takeover", "example.com", "--names", "www,api"]).assert().success();
    env.keyed()
        .args(["subnet", "192.168.1.10", "255.255.255.0", "--split", "26"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Split into /26: 4"));
    m.assert();
    split.assert();
}

#[test]
fn api_errors_say_what_to_do() {
    let env = Env::new();
    env.server.mock(|when, then| {
        when.method(GET).path("/v1/dns/example.com");
        then.status(401)
            .json_body(json!({ "error": { "code": "invalid_key", "message": "The API key isn't valid." } }));
    });
    env.server.mock(|when, then| {
        when.method(GET).path("/v1/tls/example.com");
        then.status(429)
            .header("retry-after", "1")
            .json_body(json!({ "error": { "code": "rate_limited", "reason": "per_second", "message": "Too many requests." } }));
    });
    env.keyed()
        .args(["dns", "example.com"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("The API key isn't valid."))
        .stderr(predicate::str::contains("dotquad config show"));
    env.keyed()
        .args(["tls", "example.com"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Try again in 1 second."));
}

#[test]
fn json_errors_print_the_api_error_too() {
    let env = Env::new();
    env.server.mock(|when, then| {
        when.method(GET).path("/v1/ip/10.0.0.1");
        then.status(422).json_body(
            json!({ "error": { "code": "reserved_ip", "message": "Private range.", "range": "private" } }),
        );
    });
    env.keyed()
        .args(["ip", "10.0.0.1", "--json"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"reserved_ip\""))
        .stderr(predicate::str::contains("Private range."));
}

#[test]
fn set_key_saves_a_private_file_and_checks_the_key() {
    let env = Env::new();
    let usage = env.server.mock(|when, then| {
        when.method(GET).path("/v1/usage").header("authorization", format!("Bearer {KEY}"));
        then.status(200).json_body(json!({ "plan": "free", "requests_used": 12, "requests_quota": 30000 }));
    });
    env.cmd()
        .args(["config", "set-key"])
        .write_stdin(format!("{KEY}\n"))
        .assert()
        .success()
        .stdout(predicate::str::contains("The key works: free plan, 12 of 30,000 requests"));
    usage.assert();

    let path = env.dir.path().join("config.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(KEY));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }

    // The saved key is used, and shown masked.
    env.cmd()
        .args(["config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dq_live_test…abcd"))
        .stdout(predicate::str::contains(KEY).not());
    env.cmd().args(["config", "unset-key"]).assert().success();
    env.cmd().args(["config", "show"]).assert().success().stdout(predicate::str::contains("No key saved."));
}

#[test]
fn set_key_refuses_something_that_isnt_a_key() {
    let env = Env::new();
    env.cmd()
        .args(["config", "set-key", "hunter2"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("doesn't look like a Dotquad key"));
}

#[test]
fn completions_print_a_script() {
    let env = Env::new();
    env.cmd()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#compdef dotquad"));
}
