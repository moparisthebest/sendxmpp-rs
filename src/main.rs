use std::env::args;
use std::fs::File;
use std::io::{stdin, Read, Write};
use std::iter::Iterator;
use std::path::Path;

use die::{die, Die};
use gumdrop::Options;
use serde_derive::Deserialize;

use std::process::{Command, Stdio};
use tokio_xmpp::{xmpp_stream, SimpleClient as Client};
use xmpp_parsers::message::{Body, Message};
use xmpp_parsers::presence::{Presence, Show as PresenceShow, Type as PresenceType};
use xmpp_parsers::{Element, Jid};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tls::TlsStream;

use anyhow::{anyhow, bail, Result};

#[derive(Deserialize)]
struct Config {
    jid: String,
    password: String,
}

fn parse_cfg<P: AsRef<Path>>(path: P) -> Result<Config> {
    let mut f = File::open(path)?;
    let mut input = String::new();
    f.read_to_string(&mut input)?;
    Ok(toml::from_str(&input)?)
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

    #[options(help = "Send raw XML stream, cannot be used with recipients or PGP")]
    raw: bool,

    #[options(help = "Send a <presence/> after connecting before sending messages, required for receiving for --raw")]
    presence: bool,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = args().collect();

    // Remember to skip the first argument. That's the program name.
    let opts = match MyOptions::parse_args_default(&args[1..]) {
        Ok(opts) => opts,
        Err(e) => die!("{}: {}\nUsage: {} [OPTIONS] [ARGUMENTS]\n\n{}", args[0], e, args[0], MyOptions::usage()),
    };

    if opts.help {
        die!("Usage: {} [OPTIONS] [ARGUMENTS]\n\n{}", args[0], MyOptions::usage());
    }

    let recipients: Vec<Jid> = opts.recipients.iter().map(|s| s.parse::<Jid>().die("invalid recipient jid")).collect();

    if opts.raw {
        if opts.force_pgp || opts.attempt_pgp {
            die!("--raw is incompatible with --force-pgp and --attempt-pgp");
        }
        if !recipients.is_empty() {
            die!("--raw is incompatible with recipients");
        }
    } else if recipients.is_empty() {
        die!("no recipients specified!");
    }

    let recipients = &recipients;

    let cfg = match opts.config {
        Some(config) => parse_cfg(&config).die("provided config cannot be found/parsed"),
        None => parse_cfg(dirs::config_dir().die("cannot find home directory").join("sendxmpp.toml"))
            .or_else(|_| parse_cfg("/etc/sendxmpp/sendxmpp.toml"))
            .die("valid config file not found"),
    };

    if opts.raw {
        let mut client = Client::new(&cfg.jid, &cfg.password).await.die("could not connect to xmpp server");

        if opts.presence {
            client.send_stanza(make_presence()).await.die("could not send presence");
        }

        // can paste this to test: <message xmlns="jabber:client" to="travis@burtrum.org" type="chat"><body>woot</body></message>

        pub struct OpenClient {
            pub stream: xmpp_stream::XMPPStream<TlsStream<tokio::net::TcpStream>>,
        }
        let client: OpenClient = unsafe { std::mem::transmute(client) };
        let mut open_client = client.stream.stream.try_lock().die("could not lock client stream");
        let open_client = open_client.get_mut();

        let mut rd_buf = [0u8; 256]; // todo: proper buffer size?
        let mut stdin_buf = rd_buf.clone();

        let mut stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();

        loop {
            tokio::select! {
                n = open_client.read(&mut rd_buf) => {
                    let n = n.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    stdout.write_all(&rd_buf[0..n]).await.die("could not send bytes");
                    stdout.flush().await.die("could not flush");
                },
                n = stdin.read(&mut stdin_buf) => {
                    let n = n.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    open_client.write_all(&stdin_buf[0..n]).await.die("could not send bytes");
                    open_client.flush().await.die("could not flush");
                },
            }
        }

        // Close client connection, ignoring errors
        open_client.write_all("</stream:stream>".as_bytes()).await.ok();
        open_client.flush().await.ok();
        open_client.read(&mut rd_buf).await.ok();
    } else {
        let mut data = String::new();
        stdin().lock().read_to_string(&mut data).die("error reading from stdin");
        let data = data.trim();

        let mut client = Client::new(&cfg.jid, &cfg.password).await.die("could not connect to xmpp server");

        if opts.presence {
            client.send_stanza(make_presence()).await.die("could not send presence");
        }

        for recipient in recipients {
            let reply = if opts.force_pgp || opts.attempt_pgp {
                let encrypted = gpg_encrypt(recipient.clone(), &data);
                if encrypted.is_err() {
                    if opts.force_pgp {
                        die!("pgp encryption to jid '{}' failed!", recipient);
                    } else {
                        make_reply(recipient.clone(), &data)
                    }
                } else {
                    let encrypted = encrypted.unwrap();
                    let encrypted = encrypted.trim();
                    let mut reply = make_reply(recipient.clone(), "pgp");
                    let mut x = Element::bare("x", "jabber:x:encrypted");
                    x.append_text_node(encrypted);
                    reply.append_child(x);
                    reply
                }
            } else {
                make_reply(recipient.clone(), &data)
            };
            client.send_stanza(reply).await.die("sending message failed");
        }

        // Close client connection
        client.end().await.ok(); // ignore errors here, I guess
    }
}

// Construct a <presence/>
fn make_presence() -> Element {
    let mut presence = Presence::new(PresenceType::None);
    presence.show = Some(PresenceShow::Chat);
    presence.into()
}

// Construct a chat <message/>
fn make_reply(to: Jid, body: &str) -> Element {
    let mut message = Message::new(Some(to));
    message.bodies.insert(String::new(), Body(body.to_owned()));
    message.into()
}

fn gpg_encrypt(to: Jid, body: &str) -> Result<String> {
    let to: String = std::convert::From::from(to);
    let mut gpg_cmd = Command::new("gpg")
        .arg("--encrypt")
        .arg("--armor")
        .arg("-r")
        .arg(to)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    {
        let stdin = gpg_cmd.stdin.as_mut().ok_or_else(|| anyhow!("no gpg stdin"))?;
        stdin.write_all(body.as_bytes())?;
    }

    let output = gpg_cmd.wait_with_output()?;

    if !output.status.success() {
        bail!("gpg exited with non-zero status code");
    }

    let output = output.stdout;

    // strip off headers per https://xmpp.org/extensions/xep-0027.html
    // header spec: https://tools.ietf.org/html/rfc4880#section-6.2

    // find index of leading blank line (2 newlines in a row)
    let start = first_index_of(0, &output, &[10, 10])? + 2;

    if output.len() <= start {
        bail!("length {} returned by gpg too short to be valid", output.len());
    }

    // find first newline+dash after the start
    let end = first_index_of(start, &output, &[10, 45])?;

    Ok(String::from_utf8((&output[start..end]).to_vec())?)
}

fn first_index_of(start_index: usize, haystack: &[u8], needle: &[u8]) -> Result<usize> {
    for i in start_index..haystack.len() - needle.len() + 1 {
        if haystack[i..i + needle.len()] == needle[..] {
            return Ok(i);
        }
    }
    Err(anyhow!("not found"))
}
