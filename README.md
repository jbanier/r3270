# r3270

`r3270` is a Rust TN3270/TN3270E desktop emulator prototype. The workspace is split into:

- `r3270-core`: protocol constants, Telnet/TN3270E framing, EBCDIC conversion, 3270 presentation-space engine, shared config and event types.
- `r3270-net`: async TCP/TLS session runner that connects host data to the core engine.
- `r3270-filetransfer`: IND$FILE request modeling and command construction.
- `r3270-app`: native `eframe/egui` multi-tab desktop shell.

The first implementation target is reliable display/input for SBCS 3270 with color and extended highlighting. IND$FILE, printer sessions, DBCS, scripting, and packaging are staged for later hardening.

## Development

```sh
cargo test --workspace
cargo check --workspace
cargo run -p r3270-app
```

## Validation

The workspace includes unit tests plus mock TN3270/TN3270E integration tests that run without a mainframe. When a real endpoint is available, use [docs/manual-smoke-test.md](docs/manual-smoke-test.md) and only commit sanitized golden traces.

## Packaging

Initial platform metadata lives under `packaging/`. The macOS `Info.plist` and Linux `.desktop` file are placeholders for release automation once the emulator behavior is stable enough to distribute.
