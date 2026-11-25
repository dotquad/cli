# dotquad

IP lookup, DNS, WHOIS and domain checks from your terminal, using the [Dotquad API](https://dotquad.app/docs/?ref=cli).

```console
$ dotquad 1.1.1.1
IP           1.1.1.1
Version      IPv4
Hostname     one.one.one.one           (Live reverse DNS lookup)
Location     Sydney, New South Wales   (DB-IP City Lite, 2026-11-01)
Country      AU                        (IPinfo Lite, 2026-11-25)
Coordinates  -33.8688, 151.209         (DB-IP City Lite, 2026-11-01)
Timezone     Australia/Sydney          (Timezone Boundary Builder, 2026-09-16)
Network      AS13335 Cloudflare, Inc.  (IPinfo Lite, 2026-11-25)
Prefix       1.1.1.0/24                (IPinfo Lite, 2026-11-25)
Privacy      Starter plan and up
...

$ dotquad example.com
example.com: C (65 out of 100)

F    HTTPS and headers  TLS 1.3, certificate valid for 2 months, no HSTS
A+   Email              SPF -all, DMARC reject, no DKIM key found
A    DNS                2 nameservers, IPv4 only, no CAA
A+   Registration       RESERVED-Internet Assigned Numbers Authority, registered 1995, expires in 8 months
F    DNSSEC             Broken: validating resolvers refuse to answer

To fix (11)
! DNSSEC             DNSSEC is broken
! HTTPS and headers  TLS 1.1 and TLS 1.0 still accepted
...
```

It's a single binary with no runtime, for macOS, Linux and Windows.

## Install

Not released yet. Once it is:

```sh
# Homebrew (macOS and Linux)
brew install dotquad/tap/dotquad

# npm
npm install -g dotquad

# macOS and Linux, without either
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/dotquad/cli/releases/latest/download/dotquad-installer.sh | sh

# Windows (PowerShell)
powershell -ExecutionPolicy Bypass -c "irm https://github.com/dotquad/cli/releases/latest/download/dotquad-installer.ps1 | iex"
```

Each [release](https://github.com/dotquad/cli/releases) also has the archives for macOS (Apple silicon and Intel), Linux (x64 and ARM64) and Windows (x64), with SHA-256 checksums.

To build it from source, install [Rust](https://rustup.rs) and run:

```sh
cargo install --git https://github.com/dotquad/cli
```

## Use it

Give it anything, and it works out what it is:

| Command                             | What it shows                                         |
| ----------------------------------- | ----------------------------------------------------- |
| `dotquad`                           | Your own IP address, its location and network         |
| `dotquad 1.1.1.1`                   | IP lookup for an IPv4 or IPv6 address                 |
| `dotquad example.com`               | The domain report card, graded A+ to F                |
| `dotquad AS13335`                   | ASN lookup: the network's owner, country and prefixes |
| `dotquad 10.0.0.0/22`               | Subnet calculator (CIDR)                              |
| `dotquad https://example.com/login` | Warning signs in a link                               |

Or name the check:

| Command                                                | What it shows                                                                |
| ------------------------------------------------------ | ---------------------------------------------------------------------------- |
| `dotquad dns example.com`                              | DNS records. Give it an IP for reverse DNS (PTR record)                      |
| `dotquad whois example.com`                            | Registration record for a domain, IP or ASN. `--raw` adds the registry's own |
| `dotquad tls example.com`                              | Certificate, TLS versions and security headers, graded A+ to F               |
| `dotquad email example.com`                            | SPF, DMARC, DKIM, MTA-STS, TLS-RPT and BIMI, graded A+ to F                  |
| `dotquad dnssec example.com`                           | The DNSSEC chain of trust, checked from the root                             |
| `dotquad takeover example.com`                         | Subdomains whose DNS points at something anyone could claim                  |
| `dotquad blacklist 203.0.113.9`                        | Whether an IPv4 address, or a domain's mail servers, are on a blocklist      |
| `dotquad subnet 192.168.1.10 255.255.255.0 --split 26` | Subnet calculator, with the block split into /26s                            |
| `dotquad asn AS13335 --all`                            | Every prefix, not just the first 20                                          |
| `dotquad ip 8.8.8.8 --abuse`                           | Adds the network's abuse contact from its regional registry                  |
| `dotquad usage`                                        | Your plan and this month's requests                                          |

`dotquad --help` lists every option, and `dotquad <command> --help` explains one.

### JSON for scripts

Add `--json` to any command to get the API's JSON as it is:

```sh
dotquad 8.8.8.8 --json | jq -r .network.org
dotquad check example.com --json | jq -r .grade
```

On an error, `--json` prints the API's error object to stdout, the message goes to stderr, and the exit code is 1.

### Colors

Output is colored in a terminal and plain when piped. Set `NO_COLOR=1` to turn colors off.

### Shell completions

```sh
dotquad completions zsh  > "${fpath[1]}/_dotquad"
dotquad completions bash > ~/.local/share/bash-completion/completions/dotquad
dotquad completions fish > ~/.config/fish/completions/dotquad.fish
```

## API key

Your own IP, IP lookups and the domain report card work without a key. They share the website's limit for each visitor.

Everything else needs a key. A free key covers 30,000 requests a month, and Dotquad never bills overages:

1. Get a key at [dotquad.app/signup](https://dotquad.app/signup/?ref=cli).
2. Run `dotquad config set-key` and paste it. It doesn't show as you paste.

The key is saved in `~/.config/dotquad/config.toml` (`%APPDATA%\dotquad\config.toml` on Windows), readable only by you. Or set `DOTQUAD_KEY` in your environment, which is used first.

| Command                    |                                                   |
| -------------------------- | ------------------------------------------------- |
| `dotquad config set-key`   | Save a key and check that it works                |
| `dotquad config show`      | Which key is used, masked, and where it came from |
| `dotquad config unset-key` | Remove the saved key                              |
| `dotquad config path`      | Print the config file's path                      |

The free plan includes location, network and every check above. Privacy detection (VPN, proxy, Tor and hosting) is on the Starter plan and up: [pricing](https://dotquad.app/pricing/?ref=cli).

## Privacy

The CLI sends your query and your key to `api.dotquad.app`, and nothing else. It has no telemetry. Dotquad keeps no logs of what you look up; each key's monthly request count is the only thing recorded. [Privacy Policy](https://dotquad.app/privacy/?ref=cli).

Links the CLI prints end in `?ref=cli`, so Dotquad can count visits from the CLI in total. It doesn't identify you.

## Data

IP data comes from [IPinfo](https://ipinfo.io) Lite (CC BY-SA 4.0) and [DB-IP](https://db-ip.com) City Lite (CC BY 4.0), and timezones from Timezone Boundary Builder (ODbL). Location is approximate, usually to about city level. [Credits](https://dotquad.app/attribution/?ref=cli).

## Develop

```sh
cargo test                                  # unit tests, and the binary against a mock API
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

`DOTQUAD_API_URL` points the CLI at another API, such as a local one (`http://localhost:8080`). `DOTQUAD_CONFIG` uses another config file.

### Release

Releases are built by [cargo-dist](https://opensource.axo.dev/cargo-dist/) in [release.yml](.github/workflows/release.yml), when a version tag is pushed:

1. Set the version in `Cargo.toml`, and move the changelog's Unreleased notes under `## [x.y.z] - date`.
2. Commit, then tag and push: `git tag v0.1.0 && git push origin v0.1.0`.

The workflow builds every archive and installer and makes the GitHub release. It then commits the formula to [dotquad/homebrew-tap](https://github.com/dotquad/homebrew-tap) and publishes `dotquad` to npm with Trusted Publishing. After changing `dist-workspace.toml`, run `dist generate`, then put the npm job's Trusted Publishing edits back.

## License

The code is MIT licensed ([LICENSE](LICENSE)). The Dotquad name, logo and other brand assets aren't: see [TRADEMARKS.md](TRADEMARKS.md). The Dotquad service itself is closed source; this CLI is client code.
