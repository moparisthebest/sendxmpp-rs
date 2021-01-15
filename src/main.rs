use std::env::args;
use std::fs::File;
use std::io::{stdin, Read};
use std::iter::Iterator;
use std::path::Path;

use die::{die, Die};
use gumdrop::Options;
use serde_derive::Deserialize;

use tokio_xmpp::SimpleClient as Client;
use xmpp_parsers::message::{Body, Message};
use xmpp_parsers::{Element, Jid};

#[derive(Deserialize)]
struct Config {
    jid: String,
    password: String,
}

fn parse_cfg<P: AsRef<Path>>(path: P) -> Option<Config> {
    match File::open(path) {
        Ok(mut f) => {
            let mut input = String::new();
            match f.read_to_string(&mut input) {
                Ok(_) => match toml::from_str(&input) {
                    Ok(toml) => Some(toml),
                    Err(_) => None,
                },
                Err(_) => None,
            }
        }
        Err(_) => None,
    }
}

#[derive(Default, Options)]
struct MyOptions {
    #[options(free)]
    recipients: Vec<String>,

    #[options(help = "show this help message and exit")]
    help: bool,

    #[options(
        help = "path to config file. default: ~/.config/sendxmpp.toml with fallback to /etc/sendxmpp/sendxmpp.toml"
    )]
    config: Option<String>,

    #[options(help = "Force OpenPGP encryption for all recipients", short = "e")]
    force_pgp: bool,

    #[options(help = "Attempt OpenPGP encryption for all recipients")]
    attempt_pgp: bool,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = args().collect();

    // Remember to skip the first argument. That's the program name.
    let opts = match MyOptions::parse_args_default(&args[1..]) {
        Ok(opts) => opts,
        Err(e) => die!(
            "{}: {}\nUsage: {} [OPTIONS] [ARGUMENTS]\n\n{}",
            args[0],
            e,
            args[0],
            MyOptions::usage()
        ),
    };

    if opts.help {
        die!(
            "Usage: {} [OPTIONS] [ARGUMENTS]\n\n{}",
            args[0],
            MyOptions::usage()
        );
    }

    let recipients: Vec<Jid> = opts
        .recipients
        .iter()
        .map(|s| s.parse::<Jid>().die("invalid recipient jid"))
        .collect();

    if recipients.is_empty() {
        die!("no recipients specified!");
    }

    let recipients = &recipients;

    let cfg = match opts.config {
        Some(config) => parse_cfg(&config).die("provided config cannot be found/parsed"),
        None => parse_cfg(
            dirs::config_dir()
                .die("cannot find home directory")
                .join("sendxmpp.toml"),
        )
        .or_else(|| parse_cfg("/etc/sendxmpp/sendxmpp.toml"))
        .die("valid config file not found"),
    };

    let mut data = String::new();
    stdin()
        .read_to_string(&mut data)
        .die("error reading from stdin");
    let data = data.trim();

    // Client instance
    let mut client = Client::new(&cfg.jid, &cfg.password).await.die("could not connect to xmpp server");

    for recipient in recipients {
        let reply = make_reply(recipient.clone(), &data);
        client.send_stanza(reply).await.unwrap();
    }

    // Close client connection
    client.end().await.ok(); // ignore errors here, I guess
}

// Construct a chat <message/>
fn make_reply(to: Jid, body: &str) -> Element {
    let mut message = Message::new(Some(to));
    message.bodies.insert(String::new(), Body(body.to_owned()));
    message.into()
}
