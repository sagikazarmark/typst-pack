# typst-pack documentation

New to typst-pack? Start with the [project README](../README.md).

## Guides

How to get things done.

- [CLI guide](guides/cli.md): create, inspect, compile, and extract packs
  with the `typst-pack` command.
- [Library guide](guides/library.md): pack and compile from Rust, with or
  without a filesystem.
- [OpenDAL guide](../crates/typst-pack/docs/opendal-integration.md): read and
  write packs through object storage.

## Reference

Exact behavior and guarantees.

- [API reference](https://docs.rs/typst-pack) on docs.rs.
- [Pack format](reference/pack-format.md): the `.typk` archive, its manifest,
  digests, and identities.
- [Library contract](reference/library-contract.md): what the library
  guarantees about identity, environment independence, limits, writes, and
  errors.
- [CLI parity inventory](reference/cli-parity.md): how the CLI follows and
  differs from `typst compile`.

## Development

For contributors and maintainers.

- [Embedded Typst upgrade procedure](development/embedded-typst-upgrade.md):
  moving to a new Typst release.
- [Architecture decisions](adr/): why typst-pack is built the way it is.
- [Domain glossary](../CONTEXT.md): the terms used across the code and docs.
- [Research notes](research/): historical investigations, not current
  guidance.
- [Agent instructions](agents/): how coding agents work in this repository.
