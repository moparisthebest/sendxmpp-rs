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
use std::path::Path;

use std::env::args;
use gumdrop::Options;

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
                    Err(_) => None
                },
                Err(_) => None
            }
        }
        Err(_) => None
    }
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

    let recipients: Vec<Jid> = opts.recipients.iter().map(|s| s.parse::<Jid>().expect("invalid recipient jid")).collect();

    let cfg = match opts.config {
        Some(config) => parse_cfg(&config).expect("provided config cannot be found/parsed"),
        None => parse_cfg(env::home_dir().expect("cannot find home directory").join(".config/sendxmpp.toml"))
            .or_else(|| parse_cfg("/etc/sendxmpp/sendxmpp.toml")).expect("valid config file not found")
    };

    let jid: Jid = cfg.jid.parse().expect("invalid jid in config file");

    let mut data = String::new();
    stdin().read_to_string(&mut data).expect("error reading from stdin");

    let mut client = ClientBuilder::new(jid)
        .password(cfg.password)
        .connect()
        .expect("client cannot connect");

    client.register_plugin(MessagingPlugin::new());

    for recipient in recipients {
        client.plugin::<MessagingPlugin>().send_message(&recipient, &data).expect("error sending message");
    }

    thread::spawn(|| {
        thread::sleep(Duration::from_millis(4000));
        std::process::exit(0);
    });
    client.main().expect("error during client main")
}
