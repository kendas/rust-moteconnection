# `rust-moteconnection`

The moteconnection implementation in the Rust language.

## Examples

There is currently one example in the project - the `amlistener`.

To run the example, find the address of the network device
(`localhost:9002` using the serial-forwarder protocol in this case)
and write:

```bash
cargo run --example amlistener sf@localhost:9002
```
