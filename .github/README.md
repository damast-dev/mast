# mast

The command-line companion for [Damast](https://damast.dev). **Every commit a layer.**

mast is a soft fork of [forgejo-cli](https://codeberg.org/forgejo-contrib/forgejo-cli) (`fj`). It follows every fj release and adds commands for Damast features. Everything fj can do, mast can do, against Damast and plain Forgejo alike.

## Drop-in

- Same commands and flags as `fj`; the binary is just called `mast`.
- Same config and login files, so `fj auth login` and `mast auth login` share accounts. Switching back to `fj` loses nothing.

## How the fork works

`main` = an fj release tag + mast commits. New fj releases arrive as `sync/vX.Y.Z` pull requests (`.github/workflows/upstream-sync.yml`), merged with a merge commit. Releases are tagged `vX.Y.Z-mast.N`. Upstream files only get small hooks marked `DAMAST:`; mast's own code lives in its own modules.

## License

Apache-2.0 OR MIT, like forgejo-cli. The upstream README is in [`README.md`](../README.md).
