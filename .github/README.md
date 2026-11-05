# mast

**The command line for [Damast](https://damast.dev).** What `gh` is to GitHub, `mast` wants to be for your own forge.

> [!NOTE]
> mast is a soft fork of [forgejo-cli](https://codeberg.org/forgejo-contrib/forgejo-cli) (`fj`), currently based on **fj v0.7.0**. Every new fj release arrives here as a pull request and ships as a mast release.

## Why mast exists

On GitHub, `gh` is half the workflow. You open pull requests from the terminal, check out someone's branch with one command, watch a workflow run, publish a release. Teams that move to a self-hosted forge miss that more than they expect.

`fj` already does a lot of this for Forgejo, and does it well. mast starts from `fj` and adds what Damast adds to Forgejo, so the command line knows about the same things the web UI does.

**Right now, mast is `fj` with a new name.** That is on purpose: it gives the fork a clean starting point that tracks `fj` from day one. Damast commands come next, starting with repo-bound packages.

## Drop-in, like Damast

- **Same commands, same flags.** Everything in the [fj wiki](https://codeberg.org/forgejo-contrib/forgejo-cli/wiki) works with `mast` in place of `fj`.
- **Same logins.** mast reads and writes `fj`'s config, so `fj auth login` and `mast auth login` share accounts. Use both side by side, or go back to `fj` and lose nothing.
- **Works with plain Forgejo.** Everything works against any Forgejo or Damast instance. Damast-only commands will say so clearly when the server is plain Forgejo, and never get in the way of the others.

## Install

Every release ships ready-made packages, so you don't need a Rust toolchain. mast installs as `mast` and never touches `fj`, so you can keep both.

**macOS and Linux, with Homebrew:**

```sh
brew install damast-dev/tap/mast
```

**Ubuntu and Debian (amd64, arm64):** a signed apt repository. The binary is static, so it works on any release.

```sh
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://damast-dev.github.io/apt/mast-archive-keyring.gpg | sudo tee /etc/apt/keyrings/mast-archive-keyring.gpg >/dev/null
echo "deb [signed-by=/etc/apt/keyrings/mast-archive-keyring.gpg] https://damast-dev.github.io/apt stable main" | sudo tee /etc/apt/sources.list.d/mast.list
sudo apt update && sudo apt install mast
```

**Direct download:** each [release](https://github.com/damast-dev/mast/releases) has static Linux binaries (x86_64, aarch64), macOS binaries (Apple silicon, Intel), `.deb` files and `SHA256SUMS`:

```sh
curl -fsSLO https://github.com/damast-dev/mast/releases/download/v0.7.0-mast.1/mast-0.7.0-mast.1-x86_64-unknown-linux-musl.tar.gz
tar -xzf mast-*.tar.gz mast && sudo install mast /usr/local/bin/
```

**From source** (recent stable Rust):

```sh
cargo install --locked --git https://github.com/damast-dev/mast
```

Then:

```sh
mast auth login
mast whoami
```

Releases are tagged `vX.Y.Z-mast.N` (fj version plus mast's release number). [`release.yml`](workflows/release.yml) builds and tests every package before it is published.

## Is mast for you?

If you use Damast, yes, eventually. Until the Damast commands land, `fj` does everything mast does and has a longer track record. Use whichever you like; they share their config.

## How the fork works

`main` is an fj release tag plus mast's commits. New fj releases arrive as `sync/vX.Y.Z` pull requests ([`upstream-sync.yml`](workflows/upstream-sync.yml)) and are merged with a merge commit, never rebased. Upstream files only get small hooks marked `DAMAST:`; mast's own code lives in its own modules. Anything `fj` would want goes to `fj` first.

## Thanks, fj

mast exists because [forgejo-cli](https://codeberg.org/forgejo-contrib/forgejo-cli) exists. If mast is useful to you, support `fj` and its maintainers too. The upstream README is in [`README.md`](../README.md).

## License

Apache-2.0 OR MIT, like forgejo-cli.
