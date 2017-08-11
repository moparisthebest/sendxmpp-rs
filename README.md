# sendxmpp-rs

`sendxmpp` is the XMPP equivalent of sendmail. It is an alternative to the old sendxmpp written in Perl, or the newer [sendxmpp-py](https://github.com/moparisthebest/sendxmpp-py).

Installation:  
  `cargo install`

Configuration: `cp sendxmpp.toml ~/.config/` and edit `~/.config/sendxmpp.toml` with your XMPP credentials

Usage examples:

- `echo "This is a test" | sendxmpp user@host`
- `sendxmpp user@host <README.md`

License
-------
GNU/GPLv3 - Check LICENSE.md for details