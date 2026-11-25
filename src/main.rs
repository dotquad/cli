mod api;
mod cli;
mod config;
mod detect;
mod render;
mod style;

use std::io::{IsTerminal, Read};
use std::process::ExitCode;

use anstream::{eprintln, print, println};
use anyhow::{Context, Result, bail};
use clap::{CommandFactory, Parser};
use serde_json::Value;

use api::{ApiError, Client, DEFAULT_API, DEFAULT_SITE, query, seg};
use cli::{Cli, Command, ConfigAction};
use detect::Kind;
use style::{dim, flag, ok};

/// A command that needs a key was run without one.
#[derive(Debug)]
struct NeedsKey(&'static str);

impl std::fmt::Display for NeedsKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "`dotquad {}` needs an API key.", self.0)
    }
}

impl std::error::Error for NeedsKey {}

/// Which renderer an answer goes to.
enum View {
    Lookup,
    Asn { all: bool },
    Dns,
    Whois,
    Dnssec,
    Tls,
    Email,
    Check,
    Takeover,
    Link,
    Blacklist,
    Subnet,
    Usage,
}

struct Ctx {
    json: bool,
    site: String,
    api: String,
    has_key: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut ctx = Ctx {
        json: cli.global.json,
        site: cli.global.site_url.clone().unwrap_or_else(|| DEFAULT_SITE.into()),
        api: cli.global.api_url.clone().unwrap_or_else(|| DEFAULT_API.into()),
        has_key: false,
    };
    match run(cli, &mut ctx) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            report(&err, &ctx);
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli, ctx: &mut Ctx) -> Result<()> {
    let command = match cli.command {
        Some(Command::Config { action }) => return config_cmd(action, ctx),
        Some(Command::Completions { shell }) => {
            clap_complete::generate(shell, &mut Cli::command(), "dotquad", &mut std::io::stdout());
            return Ok(());
        }
        Some(c) => c,
        None => match cli.query {
            None => Command::Me { abuse: false },
            Some(q) => from_query(q)?,
        },
    };

    let key = config::key()?.map(|(k, _)| k);
    ctx.has_key = key.is_some();
    let client = Client::new(&ctx.api, key);
    let (value, view) = fetch(&client, command)?;

    if ctx.json {
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    let site = ctx.site.as_str();
    let text = match view {
        View::Lookup => render::lookup(&value, site),
        View::Asn { all } => render::asn(&value, site, all),
        View::Dns => render::dns(&value, site),
        View::Whois => render::whois(&value, site),
        View::Dnssec => render::dnssec(&value, site),
        View::Tls => render::tls(&value, site),
        View::Email => render::email(&value, site),
        View::Check => render::check(&value, site),
        View::Takeover => render::takeover(&value, site),
        View::Link => render::link(&value, site),
        View::Blacklist => render::blacklist(&value, site),
        View::Subnet => render::subnet(&value, site),
        View::Usage => render::usage(&value, site),
    };
    print!("{text}");
    Ok(())
}

/// `dotquad <query>`: picks the command from what the query looks like.
fn from_query(q: String) -> Result<Command> {
    let q = q.trim().to_string();
    Ok(match detect::detect(&q) {
        Some(Kind::Ipv4 | Kind::Ipv6) => Command::Ip { ip: q, abuse: false },
        Some(Kind::Domain) => Command::Check { domain: q },
        Some(Kind::Asn) => Command::Asn { asn: q, all: false },
        Some(Kind::Cidr) => Command::Subnet { block: vec![q], split: None },
        Some(Kind::Link) => Command::Link { url: q },
        None => bail!(
            "Not an IP address, domain, ASN, CIDR block or link: {q}\nSee `dotquad --help` for the commands."
        ),
    })
}

fn fetch(c: &Client, command: Command) -> Result<(Value, View)> {
    let keyed = c.has_key();
    let need = |name: &'static str| -> Result<()> { if keyed { Ok(()) } else { Err(NeedsKey(name).into()) } };
    let abuse = |on: bool| if on { "?abuse=true" } else { "" };
    let v = match command {
        // These three also work without a key, through the open terminal endpoints.
        Command::Me { abuse: a } => {
            let v = if keyed {
                c.v1(&format!("/me{}", abuse(a)))?
            } else if a {
                return Err(NeedsKey("me --abuse").into());
            } else {
                c.open("/json")?
            };
            (v, View::Lookup)
        }
        Command::Ip { ip, abuse: a } => {
            let v = if keyed {
                c.v1(&format!("/ip/{}{}", seg(&ip), abuse(a)))?
            } else if a {
                return Err(NeedsKey("ip --abuse").into());
            } else {
                c.open(&format!("/{}/json", seg(&ip)))?
            };
            (v, View::Lookup)
        }
        Command::Check { domain } => {
            let v = if keyed {
                c.v1(&format!("/check/{}", seg(&domain)))?
            } else {
                c.open(&format!("/check/{}/json", seg(&domain)))?
            };
            (v, View::Check)
        }
        Command::Asn { asn, all } => {
            need("asn")?;
            (c.v1(&format!("/asn/{}", seg(&asn)))?, View::Asn { all })
        }
        Command::Dns { query: q } => {
            need("dns")?;
            (c.v1(&format!("/dns/{}", seg(&q)))?, View::Dns)
        }
        Command::Whois { query: q, raw } => {
            need("whois")?;
            let raw = if raw { "?raw=true" } else { "" };
            (c.v1(&format!("/whois/{}{raw}", seg(&q)))?, View::Whois)
        }
        Command::Dnssec { name } => {
            need("dnssec")?;
            (c.v1(&format!("/dnssec/{}", seg(&name)))?, View::Dnssec)
        }
        Command::Tls { host } => {
            need("tls")?;
            (c.v1(&format!("/tls/{}", seg(&host)))?, View::Tls)
        }
        Command::Email { domain, selectors } => {
            need("email")?;
            let q = query(&[("selectors", Some(selectors.join(",")))]);
            (c.v1(&format!("/email/{}{q}", seg(&domain)))?, View::Email)
        }
        Command::Takeover { domain, names } => {
            need("takeover")?;
            let q = query(&[("names", Some(names.join(",")))]);
            (c.v1(&format!("/takeover/{}{q}", seg(&domain)))?, View::Takeover)
        }
        Command::Link { url } => {
            need("link")?;
            (c.v1(&format!("/link{}", query(&[("url", Some(url))])))?, View::Link)
        }
        Command::Blacklist { query: q } => {
            need("blacklist")?;
            (c.v1(&format!("/blacklist/{}", seg(&q)))?, View::Blacklist)
        }
        Command::Subnet { block, split } => {
            need("subnet")?;
            let q = query(&[("split_prefix", split.map(|s| s.to_string()))]);
            (c.v1(&format!("/subnet/{}{q}", seg(&block.join(" "))))?, View::Subnet)
        }
        Command::Usage => {
            need("usage")?;
            (c.v1("/usage")?, View::Usage)
        }
        Command::Config { .. } | Command::Completions { .. } => unreachable!("handled in run"),
    };
    Ok(v)
}

fn config_cmd(action: ConfigAction, ctx: &Ctx) -> Result<()> {
    match action {
        ConfigAction::Path => println!("{}", config::path()?.display()),
        ConfigAction::Show => match config::key()? {
            Some((key, source)) => {
                let from = match source {
                    config::KeySource::Env => "from DOTQUAD_KEY".to_string(),
                    config::KeySource::File => format!("from {}", config::path()?.display()),
                };
                println!("{}  {}", config::mask(&key), dim(&format!("({from})")));
            }
            None => println!(
                "No key saved. Get a free one at {}, then run `dotquad config set-key`.",
                signup(&ctx.site)
            ),
        },
        ConfigAction::UnsetKey => {
            let mut file = config::load()?;
            if file.key.take().is_none() {
                println!("No key saved.");
            } else {
                let path = config::save(&file)?;
                println!("Removed the key from {}.", path.display());
            }
        }
        ConfigAction::SetKey { key } => {
            let key = match key {
                Some(k) => k,
                None if std::io::stdin().is_terminal() => {
                    rpassword::prompt_password("Paste your API key (it won't show): ")
                        .context("Couldn't read the key")?
                }
                None => {
                    let mut s = String::new();
                    std::io::stdin().read_to_string(&mut s).context("Couldn't read the key")?;
                    s
                }
            };
            let key = key.trim().to_string();
            config::check_key_format(&key)?;
            let mut file = config::load()?;
            file.key = Some(key.clone());
            let path = config::save(&file)?;
            println!("Saved {} to {}.", config::mask(&key), path.display());
            if std::env::var("DOTQUAD_KEY").is_ok_and(|k| !k.trim().is_empty()) {
                println!("{}", flag("DOTQUAD_KEY is set in this shell, and it's used before the saved key."));
            }
            // Checking usage doesn't count as a request.
            match Client::new(&ctx.api, Some(key)).v1("/usage") {
                Ok(u) => {
                    let plan = u.get("plan").and_then(Value::as_str).unwrap_or("?");
                    let used = u.get("requests_used").and_then(Value::as_u64).unwrap_or(0);
                    let quota = u.get("requests_quota").and_then(Value::as_u64).unwrap_or(0);
                    println!(
                        "{} {plan} plan, {} of {} requests used this month.",
                        ok("The key works:"),
                        render::thousands(used),
                        render::thousands(quota)
                    );
                }
                Err(e) if e.code == "invalid_key" => {
                    println!("{} {}", flag("The API doesn't accept this key:"), e.message);
                }
                Err(_) => println!("{}", dim("Couldn't reach the API to check the key.")),
            }
        }
    }
    Ok(())
}

fn signup(site: &str) -> String {
    format!("{}/signup/?ref=cli", site.trim_end_matches('/'))
}

fn report(err: &anyhow::Error, ctx: &Ctx) {
    let label = flag("error:");
    if err.downcast_ref::<NeedsKey>().is_some() {
        eprintln!("{label} {err}");
        eprintln!("A free key covers 30,000 requests a month.");
        eprintln!("  1. Get one: {}", signup(&ctx.site));
        eprintln!("  2. Run: dotquad config set-key");
        eprintln!("{}", dim("Or set DOTQUAD_KEY in your environment."));
        return;
    }
    let Some(e) = err.downcast_ref::<ApiError>() else {
        eprintln!("{label} {err:#}");
        return;
    };
    if ctx.json
        && let Some(body) = &e.body
    {
        println!("{}", serde_json::to_string_pretty(body).unwrap_or_default());
    }
    eprintln!("{label} {}", e.message);
    let site = ctx.site.trim_end_matches('/');
    match e.code.as_str() {
        "invalid_key" => {
            eprintln!("Check which key is used with `dotquad config show`.");
            eprintln!("Your key is on the dashboard: {site}/dashboard/?ref=cli");
        }
        "plan_required" => {
            if let Some(plan) = &e.plan {
                eprintln!("This needs the {plan} plan.");
            }
            let plans = e.upgrade_url.clone().unwrap_or_else(|| format!("{site}/pricing/?ref=cli"));
            eprintln!("Plans: {plans}");
        }
        "rate_limited" if e.reason.as_deref() != Some("quota") => {
            if let Some(s) = e.retry_after {
                eprintln!("Try again in {s} second{}.", if s == 1 { "" } else { "s" });
            }
            if !ctx.has_key {
                eprintln!("Without a key, this shares the website's limit for each visitor.");
                eprintln!("A free key covers 30,000 requests a month: {}", signup(&ctx.site));
            }
        }
        _ => {}
    }
}
