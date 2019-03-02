use std::env::args;
use std::fs::File;
use std::io::{stdin, Read};
use std::iter::Iterator;
use std::path::Path;

use die::{die, Die};
use gumdrop::Options;
use serde_derive::Deserialize;

use futures::{future, Sink, Stream};
use tokio::runtime::current_thread::Runtime;
use tokio_xmpp::xmpp_codec::Packet;
use tokio_xmpp::Client;
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

fn main() {
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

    // tokio_core context
    let mut rt = Runtime::new().die("unknown error");
    // Client instance
    let client = Client::new(&cfg.jid, &cfg.password).die("could not connect to xmpp server");

    // Make the two interfaces for sending and receiving independent
    // of each other so we can move one into a closure.
    let (sink, stream) = client.split();
    let mut sink_state = Some(sink);

    // Main loop, processes events
    let done = stream.for_each(move |event| {
        if event.is_online() {
            let mut sink = sink_state.take().die("unknown error");
            for recipient in recipients {
                let reply = make_reply(recipient.clone(), &data);
                sink.start_send(Packet::Stanza(reply)).die("send failed");
            }
            sink.start_send(Packet::StreamEnd)
                .die("send stream end failed");
        }

        Box::new(future::ok(()))
    });

    // Start polling `done`
    match rt.block_on(done) {
        Ok(_) => (),
        Err(e) => die!("Fatal: {}", e),
    };
}

// Construct a chat <message/>
fn make_reply(to: Jid, body: &str) -> Element {
    let mut message = Message::new(Some(to));
    message.bodies.insert(String::new(), Body(body.to_owned()));
    message.into()
}
