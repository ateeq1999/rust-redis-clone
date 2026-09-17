# How `parse_array` works

`parse_array` lives in [`src/resp/types.rs`](src/resp/types.rs) and turns a RESP
Array (`*<count>\r\n<element-1>...<element-n>`) into a `RespType::Array(Vec<RespType>)`.
This is the frame Redis commands are sent in — e.g. `ECHO hello` is really an
array of two bulk strings.

We'll trace it against one concrete input, the payload
[`examples/client.rs`](examples/client.rs) sends for its "Array (RESP command)"
test case:

```
*2\r\n$4\r\nECHO\r\n$5\r\nhello\r\n
```

Laid out with byte indices:

```
index:  0  1  2  3  4  5  6  7  8  9  10 11 12 13 14 15 16 17 18 19 20 21 22 23 24
byte:   *  2  \r \n $  4  \r \n E  C  H  O  \r \n $  5  \r \n h  e  l  l  o  \r \n
```

So `buffer.len() == 25`. Two RESP values are packed back-to-back inside the
array: `$4\r\nECHO\r\n` (a 10-byte bulk string, indices 4-13) and
`$5\r\nhello\r\n` (an 11-byte bulk string, indices 14-24).

## The code

```rust
fn parse_array(buffer: &[u8]) -> Result<(RespType, usize), RespError> {
    // 1. Find the first CRLF to extract the element-count line.
    let (len_bytes, length_line_size) = match Self::read_till_crlf(&buffer[1..]) {
        Some(result) => result,
        None => return Err(RespError::Incomplete),
    };

    // 2. Parse the number of elements the array should contain
    let num_elements = Self::parse_usize_from_buf(len_bytes)?;

    // +1 accounts for the leading '*' byte
    let mut consumed = 1 + length_line_size;
    let mut elements = Vec::with_capacity(num_elements);

    // 3. Parse each element in turn, advancing past whatever bytes it consumed.
    for _ in 0..num_elements {
        let (element, element_size) = Self::parse(&buffer[consumed..])?;
        elements.push(element);
        consumed += element_size;
    }

    Ok((RespType::Array(elements), consumed))
}
```

## Step by step

### Step 1 — find the count line

```rust
let (len_bytes, length_line_size) = match Self::read_till_crlf(&buffer[1..]) {
    Some(result) => result,
    None => return Err(RespError::Incomplete),
};
```

`buffer[0]` is `'*'` (that's already how `RespType::parse` routed us here), so
we skip it and scan `buffer[1..]` for the first `\r\n`. `read_till_crlf`
returns everything *before* the CRLF plus the total bytes it consumed
(including the CRLF itself).

For our example, `buffer[1..]` is:

```
2\r\n$4\r\nECHO\r\n$5\r\nhello\r\n
```

The first CRLF is right after the `2`, so:

- `len_bytes` = `b"2"`
- `length_line_size` = `3` (the `2`, plus the 2 bytes of `\r\n`)

If the buffer had ended mid-way through the count line — say the socket had
only delivered `*2` so far with no trailing CRLF yet — `read_till_crlf` would
return `None` and we'd bail out with `RespError::Incomplete`. That's not a
protocol error; it tells the codec in [`src/resp/codec.rs`](src/resp/codec.rs)
"not enough bytes yet, come back after the next read".

### Step 2 — parse the element count

```rust
let num_elements = Self::parse_usize_from_buf(len_bytes)?;
```

`parse_usize_from_buf(b"2")` converts the ASCII digits to a `usize`:
`num_elements = 2`. If the bytes weren't valid UTF-8 or didn't parse as a
number (e.g. `*abc\r\n`), this returns `RespError::Other(..)` — a real
protocol error, not `Incomplete`, since arriving bytes can never fix
"abc" into a number.

### Step 3 — set up the consume cursor

```rust
let mut consumed = 1 + length_line_size;
let mut elements = Vec::with_capacity(num_elements);
```

`consumed` tracks how many bytes of `buffer` we've accounted for so far, so
the *next* element always starts at `buffer[consumed..]`.

- `1` — the leading `*` byte we skipped in Step 1
- `length_line_size` — the `2\r\n` we just consumed (3 bytes)

So `consumed = 1 + 3 = 4`. That's exactly the index where `$4\r\nECHO\r\n`
begins in the byte table above — index 4.

`elements` is pre-sized to `num_elements` (2) since we already know how many
we're about to push.

### Step 4 — parse each element, advancing the cursor

```rust
for _ in 0..num_elements {
    let (element, element_size) = Self::parse(&buffer[consumed..])?;
    elements.push(element);
    consumed += element_size;
}
```

This is the recursive heart of it: each element can be *any* RESP type
(bulk string, simple string, even a nested array), so we just hand
`buffer[consumed..]` back to `Self::parse` — the same dispatcher that routed
us into `parse_array` in the first place — and let it figure out what's
there.

**Iteration 1** (`consumed == 4`):

`buffer[4..]` is `$4\r\nECHO\r\nhello...` (everything from index 4 onward).
`Self::parse` sees `$` and calls `parse_bulk_string`, which reads the length
line (`4`), then reads exactly 4 payload bytes (`ECHO`), then checks for the
trailing `\r\n`. It returns:

```
element      = RespType::BulkString("ECHO")
element_size = 10   // "$4\r\nECHO\r\n" is 10 bytes
```

We push `BulkString("ECHO")` onto `elements` and update:

```
consumed = 4 + 10 = 14
```

Index 14 in the byte table is exactly where `$5\r\nhello\r\n` starts — the
cursor lines up perfectly because `element_size` is "bytes this element
occupied in the buffer", not "characters in the string".

**Iteration 2** (`consumed == 14`):

`buffer[14..]` is `$5\r\nhello\r\n`. `parse_bulk_string` reads length `5`,
payload `hello`, and the trailing CRLF. It returns:

```
element      = RespType::BulkString("hello")
element_size = 11   // "$5\r\nhello\r\n" is 11 bytes
```

We push `BulkString("hello")` and update:

```
consumed = 14 + 11 = 25
```

The loop has now run `num_elements` (2) times, so it ends.

If the buffer had been cut short — say the client had only sent
`$5\r\nhel` so far — `parse_bulk_string` would detect the payload is shorter
than expected and return `Err(RespError::Incomplete)`. The `?` on
`Self::parse(&buffer[consumed..])?` propagates that straight out of
`parse_array` too: **the whole array is only "ready" once every one of its
elements is fully buffered**, no matter how deep the nesting goes.

### Step 5 — return the assembled array

```rust
Ok((RespType::Array(elements), consumed))
```

Final result for our example:

```rust
(
    RespType::Array(vec![
        RespType::BulkString("ECHO".to_string()),
        RespType::BulkString("hello".to_string()),
    ]),
    25, // total bytes consumed out of the original buffer
)
```

`25` is the full length of the input (`*2\r\n` is 4 bytes, plus the two
bulk-string elements at 10 and 11 bytes each: `4 + 10 + 11 = 25`) — every byte
in the buffer was accounted for. The caller (typically
[`RespCodec::decode`](src/resp/codec.rs)) uses that `25` to advance its
`BytesMut` past exactly the bytes this array consumed, leaving anything
pipelined after it (e.g. a second command in the same TCP read) intact for
the next call to `decode`.
