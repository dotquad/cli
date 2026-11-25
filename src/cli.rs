use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;

const AFTER_HELP: &str = "\
Examples:
  dotquad                     Your own IP address
  dotquad 1.1.1.1             IP lookup
  dotquad example.com         Domain report card, graded A+ to F
  dotquad AS13335             ASN lookup
  dotquad 10.0.0.0/22         Subnet calculator (CIDR)
  dotquad dns example.com     DNS records
  dotquad 8.8.8.8 --json      The API's JSON, for jq and scripts

Your own IP, IP lookups and the report card work without a key, within a per-minute limit.
Everything else needs a free key (30,000 requests a month): https://dotquad.app/signup/?ref=cli
Then run `dotquad config set-key`, or set DOTQUAD_KEY.";

/// IP lookup, DNS, WHOIS and domain checks from the Dotquad API.
#[derive(Debug, Parser)]
#[command(name = "dotquad", version, args_conflicts_with_subcommands = true, after_help = AFTER_HELP)]
pub struct Cli {
    /// An IP address, domain, ASN (AS13335), CIDR block or link. Leave it out for your own IP.
    pub query: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub global: Global,
}

#[derive(Debug, Args)]
pub struct Global {
    /// Print the API's JSON instead of a table
    #[arg(long, global = true)]
    pub json: bool,

    /// The API's address, for testing
    #[arg(long, global = true, env = "DOTQUAD_API_URL", hide = true)]
    pub api_url: Option<String>,

    /// The website's address, for links
    #[arg(long, global = true, env = "DOTQUAD_SITE_URL", hide = true)]
    pub site_url: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Your own IP address: location, network and hostname
    Me {
        /// Add the network's abuse contact from its regional registry
        #[arg(long)]
        abuse: bool,
    },
    /// IP lookup: location, network and hostname for an IPv4 or IPv6 address
    Ip {
        ip: String,
        /// Add the network's abuse contact from its regional registry
        #[arg(long)]
        abuse: bool,
    },
    /// ASN lookup: a network's owner, country and announced prefixes
    Asn {
        /// AS13335 or 13335
        asn: String,
        /// List every prefix, not just the first 20
        #[arg(long)]
        all: bool,
    },
    /// DNS records for a name, or reverse DNS (PTR record) for an IP
    Dns { query: String },
    /// Registration record for a domain, IP or ASN
    Whois {
        query: String,
        /// Add the registry's own record
        #[arg(long)]
        raw: bool,
    },
    /// DNSSEC chain of trust, checked from the root
    Dnssec { name: String },
    /// A site's HTTPS: certificate, TLS versions and security headers, graded A+ to F
    Tls { host: String },
    /// A domain's email authentication: SPF, DMARC, DKIM, MTA-STS, TLS-RPT and BIMI
    Email {
        domain: String,
        /// More DKIM selectors to check, comma-separated
        #[arg(long, value_delimiter = ',')]
        selectors: Vec<String>,
    },
    /// Domain report card: HTTPS, email, DNS, registration and DNSSEC, graded A+ to F
    Check { domain: String },
    /// Subdomain takeover check, from DNS
    Takeover {
        domain: String,
        /// More subdomain names to check, comma-separated
        #[arg(long, value_delimiter = ',')]
        names: Vec<String>,
    },
    /// Warning signs in a web link: look-alike names, redirects, site age and certificate
    Link { url: String },
    /// Whether an IPv4 address, or a domain's mail servers, are on a blocklist
    Blacklist { query: String },
    /// Subnet calculator (CIDR): network, netmask, host range and count
    Subnet {
        /// 10.0.0.0/24, 2001:db8::/48, or an address and netmask (192.168.1.10 255.255.255.0)
        #[arg(required = true, num_args = 1..=2)]
        block: Vec<String>,
        /// List the smaller subnets of this prefix length
        #[arg(long, value_name = "PREFIX")]
        split: Option<u8>,
    },
    /// Your plan and this month's requests
    Usage,
    /// Set, show or remove your API key
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Print a shell completion script
    Completions { shell: Shell },
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Save your API key. Leave KEY out to paste it without it showing.
    SetKey { key: Option<String> },
    /// Remove the saved key
    UnsetKey,
    /// Show where the key comes from, masked
    Show,
    /// Print the config file's path
    Path,
}
