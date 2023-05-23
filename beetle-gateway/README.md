# beetle gateway

[![crates.io](https://img.shields.io/crates/v/beetle-gateway.svg?style=flat-square)](https://crates.io/crates/beetle-gateway)
[![Released API docs](https://img.shields.io/docsrs/beetle-gateway?style=flat-square)](https://docs.rs/beetle-gateway)
[![MIT/Apache-2.0 licensed](https://img.shields.io/crates/l/beetle-gateway?style=flat-square)](../LICENSE-MIT)
[![CI](https://img.shields.io/github/workflow/status/n0-computer/beetle/Continuous%20integration?style=flat-square)](https://github.com/n0-computer/beetle/actions?query=workflow%3A%22Continuous+integration%22)

A Rust implementation of an [IPFS gateway](https://docs.ipfs.tech/concepts/ipfs-gateway/) based on
[beetle](https://github.com/n0-computer/beetle). An IPFS gateway allows you to
access content on the IPFS network over HTTP.

## Running / Building

`cargo run -- -p 10000`

### Options

- Run with `cargo run -- -h` for details
- `-wcf` Writeable, Cache, Fetch (options to toggle write enable, caching mechanics and fetching from the network); currently exists but is not implemented
- `-p` Port the gateway should listen on

## ENV Variables

- `BEETLE_INSTANCE_ID` - unique instance identifier, preferably some name than hard id (default: generated lower & snake case name)
- `BEETLE_ENV` - indicates the service environment (default: `dev`)

## Endpoints

| Endpoint                          | Flag                                       | Description                                                                             | Default     |
|-----------------------------------|--------------------------------------------|-----------------------------------------------------------------------------------------|-------------|
| `/ipfs/:cid` & `/ipfs/:cid/:path` | `?format={"", "fs", "raw", "car"}`         | Specifies the serving format & content-type                                             | `""/fs`     |
|                                   | `?filename=DESIRED_FILE_NAME`              | Specifies a filename for the attachment                                                 | `{cid}.bin` |
|                                   | `?download={true, false}`                  | Sets content-disposition to attachment, browser prompts to save file instead of loading | `false`     |
|                                   | `?force_dir={true, false}`                 | Lists unixFS directories even if they contain an `index.html` file                      | `false`     |
|                                   | `?uri=ENCODED_URL`                         | Query parameter to handle navigator.registerProtocolHandler Web API ie. ipfs://         | `""`        |


## License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br/>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
