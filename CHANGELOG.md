# Changelog

## [Unreleased]

### Added

- `dotquad <query>` works out whether the query is an IP, domain, ASN, CIDR block or link, and runs the matching check.
- Commands for IP lookup (`me`, `ip`), `asn`, `dns`, `whois`, `dnssec`, `tls`, `email`, `check`, `takeover`, `link`, `blacklist`, `subnet` and `usage`.
- `--json` on every command prints the API's JSON.
- Your own IP, IP lookups and the report card work without a key, through the open terminal endpoints.
- `dotquad config set-key` saves the key to a file only you can read and checks it. `DOTQUAD_KEY` is used first.
- Shell completions for bash, zsh, fish, PowerShell and Elvish.
