#[macro_use]
extern crate serde_derive;
extern crate toml;
extern crate gumdrop;
#[macro_use]
extern crate gumdrop_derive;
extern crate xmpp;

use xmpp::jid::Jid;
use xmpp::client::ClientBuilder;
use xmpp::plugins::messaging::MessagingPlugin;

use std::time::Duration;
use std::thread;
use std::env;
use std::iter::Iterator;
use std::io::{Read, stdin};
use std::fs::File;

use std::env::args;
use gumdrop::Options;

#[derive(Deserialize)]
struct Config {
    jid: String,
    password: String,
}

fn parse_cfg(path: &str) -> Option<Config> {
    File::open(path).and_then(|mut f| {
        let mut input = String::new();
        f.read_to_string(&mut input)?;
        match toml::from_str(&input) {
            Ok(toml) => Ok(toml),
            Err(error) => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, error))
        }
    }).ok()
}

#[derive(Default, Options)]
struct MyOptions {
    #[options(free)]
    recipients: Vec<String>,

    #[options(help = "show this help message and exit")]
    help: bool,

    #[options(help = "path to config file. default: ~/.config/sendxmpp.toml with fallback to /etc/sendxmpp/sendxmpp.toml")]
    config: Option<String>,

    #[options(help = "Force OpenPGP encryption for all recipients", short = "e")]
    force_pgp: bool,

    #[options(help = "Attempt OpenPGP encryption for all recipients")]
    attempt_pgp: bool,
}

fn main() {
    let args: Vec<String> = args().collect();

    // Remember to skip the first argument. That's the program name.
    let opts = match MyOptions::parse_args_default(&args[1..]) {
        Ok(opts) => opts,
        Err(e) => {
            println!("{}: {}", args[0], e);
            println!("Usage: {} [OPTIONS] [ARGUMENTS]", args[0]);
            println!();
            println!("{}", MyOptions::usage());
            return;
        }
    };

    if opts.help {
        println!("Usage: {} [OPTIONS] [ARGUMENTS]", args[0]);
        println!();
        println!("{}", MyOptions::usage());
        return;
    }

    let recipients: Vec<Jid> = opts.recipients.iter().map(|s| s.parse::<Jid>().unwrap()).collect();

    let cfg = match opts.config {
        Some(config) => parse_cfg(&config).unwrap(),
        None => {
            let mut home_cfg = String::new();
            home_cfg += env::home_dir().unwrap().to_str().unwrap();
            home_cfg += "/.config/sendxmpp.toml";

            parse_cfg(&home_cfg).unwrap_or_else(|| parse_cfg("/etc/sendxmpp/sendxmpp.toml").unwrap())
        }
    };

    let jid: Jid = cfg.jid.parse().unwrap();

    let mut data = String::new();
    stdin().read_to_string(&mut data).expect("error reading from stdin");

    let mut client = ClientBuilder::new(jid)
        .password(cfg.password)
        .connect()
        .unwrap();

    client.register_plugin(MessagingPlugin::new());

    for recipient in recipients {
        client.plugin::<MessagingPlugin>().send_message(&recipient, &data).unwrap();
    }

    thread::spawn(|| {
        thread::sleep(Duration::from_millis(4000));
        std::process::exit(0);
    });
    client.main().unwrap()
}
