# nimblecache

A small Redis clone built from scratch in Rust with [tokio](https://tokio.rs/),
as a learning project for the RESP protocol and async networking. It speaks
enough of the real Redis wire protocol that `redis-cli` can talk to it.

## What's implemented

- **RESP2 parsing and encoding** ([`src/resp/`](src/resp/)) - `SimpleString`,
  `BulkString`, `SimpleError`, `Array`, `Integer`, and `NullBulkString` (the
  `$-1\r\n` Redis uses for "no value"), all parsed incrementally so a value
  split across multiple TCP reads is handled correctly.
- **Stream framing via `tokio_util::codec`** ([`src/resp/codec.rs`](src/resp/codec.rs)) -
  `RespCodec` implements `Decoder`/`Encoder` so each connection is driven
  through a `Framed<TcpStream, RespCodec>` instead of manual buffer
  bookkeeping. Connections are persistent: a client can send many commands,
  pipelined or one at a time, over the same socket.
- **Command dispatch** ([`src/command/`](src/command/)) - every RESP `Array`
  received from a client is interpreted as `<command name> <args...>` and
  matched case-insensitively. Anything else at the top level (a bare
  `SimpleString`, for instance) or an unrecognized command name gets back a
  RESP `SimpleError`, the same way real Redis responds to malformed input.
- **An in-memory key-value store** ([`src/storage.rs`](src/storage.rs)) -
  a `HashMap` behind an `Arc<RwLock<_>>`, shared cheaply (`Clone` just clones
  the `Arc`) across every connection task. Values are either a `String` or a
  `VecDeque<String>` (a list), and using a list command on a string key (or
  vice versa) returns Redis's real `WRONGTYPE` error.

### Supported commands

| Command | Usage | Reply |
|---|---|---|
| `PING` | `PING` | `+PONG` |
| `PING` | `PING <message>` | `<message>` as a bulk string |
| `SET` | `SET <key> <value>` | `+OK` |
| `GET` | `GET <key>` | the value as a bulk string, or `$-1` (null) if the key isn't set |
| `LPUSH` | `LPUSH <key> <value> [value ...]` | the list's length, as an integer |
| `RPUSH` | `RPUSH <key> <value> [value ...]` | the list's length, as an integer |
| `LRANGE` | `LRANGE <key> <start> <stop>` | the elements in that range, as an array of bulk strings |

`SET` doesn't support Redis's advanced options (`EX`, `PX`, `NX`, ...) yet -
calling it with anything other than exactly a key and a value returns an
error rather than silently ignoring the extra arguments.

`LRANGE`'s `start`/`stop` are zero-based and inclusive; negative indices
count from the end of the list (`-1` is the last element), and an
out-of-range index is clamped rather than treated as an error - `LRANGE key 0
-1` always returns the whole list, matching real Redis.

## Running it

Start the server (listens on `127.0.0.1:6300`):

```sh
cargo run
```

Point a real Redis client at it:

```sh
redis-cli -p 6300
127.0.0.1:6300> PING
PONG
127.0.0.1:6300> SET foo bar
OK
127.0.0.1:6300> GET foo
"bar"
127.0.0.1:6300> GET missing
(nil)
127.0.0.1:6300> RPUSH mylist a b c
(integer) 3
127.0.0.1:6300> LRANGE mylist 0 -1
1) "a"
2) "b"
3) "c"
```

Or run the bundled example client, which exercises the server end-to-end
(PING, SET/GET, list commands, an unknown command, and a couple of
malformed-input cases) without needing `redis-cli` installed:

```sh
cargo run --example client
```

Set `RUST_LOG=info` on either command to see the server's/client's log
output.

## Project layout

```
src/
  main.rs           TCP server: accepts connections, frames RESP values,
                     dispatches them as commands
  resp.rs            RespError
  resp/
    types.rs         RespType and its parser/encoder
    codec.rs          RespCodec (tokio_util Decoder/Encoder)
  command.rs          Command, CommandError, and dispatch by command name
  command/
    ping.rs, set.rs, get.rs, push.rs (LPUSH/RPUSH), lrange.rs
  storage.rs           KeyValueStore, the shared in-memory store
examples/
  client.rs            a standalone RESP client used to exercise the server
  read_till_crlf.rs     a scratch file exploring the CRLF-scanning logic
```

See [parse-array.md](parse-array.md) for a byte-by-byte walkthrough of how a
RESP array (i.e. a command) gets parsed.

## Testing

There's no `#[test]` suite yet - `cargo run --example client` is the current
way to exercise the server. `cargo build --all-targets` and
`cargo clippy --all-targets` are both clean.
