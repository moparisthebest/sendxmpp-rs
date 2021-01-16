# sendxmpp-rs

`sendxmpp` is the XMPP equivalent of sendmail. It is an alternative to the old sendxmpp written in Perl, or the newer [sendxmpp-py](https://github.com/moparisthebest/sendxmpp-py).

Installation:
  `cargo install sendxmpp`

Configuration: `cp sendxmpp.toml ~/.config/` and edit `~/.config/sendxmpp.toml` with your XMPP credentials

```
Usage: sendxmpp [OPTIONS] [ARGUMENTS]

Positional arguments:
  recipients

Optional arguments:
  -h, --help           show this help message and exit
  -c, --config CONFIG  path to config file. default: ~/.config/sendxmpp.toml with fallback to /etc/sendxmpp/sendxmpp.toml
  -e, --force-pgp      Force OpenPGP encryption for all recipients
  -a, --attempt-pgp    Attempt OpenPGP encryption for all recipients
```

Usage examples:

- `echo "This is a test" | sendxmpp user@host`
- `sendxmpp user@host <README.md`

License
-------
GNU/AGPLv3 - Check LICENSE.md for details
