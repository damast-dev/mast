# mast

**The command line for [Damast](https://damast.dev).** What `gh` is to GitHub, `mast` wants to be for your own forge.

> [!NOTE]
> mast is a soft fork of [forgejo-cli](https://codeberg.org/forgejo-contrib/forgejo-cli) (`fj`), currently based on **fj v0.7.0**. Every new fj release arrives here as a pull request and ships as a mast release.

## Why mast exists

On GitHub, `gh` is half the workflow. You open pull requests from the terminal, check out someone's branch with one command, watch a workflow run, publish a release. Teams that move to a self-hosted forge miss that more than they expect.

`fj` already does a lot of this for Forgejo, and does it well. mast starts from `fj` and adds what Damast adds to Forgejo, so the command line knows about the same things the web UI does.

**Right now, mast is `fj` with a new name.** That is on purpose: it gives the fork a clean starting point that tracks `fj` from day one. Damast commands come next, starting with repo-bound packages.

## What mast adds

### Repo-bound packages

```sh
mast package list acme                                   # one line per package: versions, newest, repository
mast package info acme npm tool                          # versions, linked repository, repository access
mast package link acme npm tool tool-repo                # link to a repository of the same owner
mast package unlink acme npm tool
mast package inherit acme npm tool off                   # repository readers and writers no longer count
mast package settings acme --new-packages repo_writers   # who may create new packages (default: owners)
mast package delete acme npm tool 1.0.0                  # one version
```

`list`, `info`, `link`, `unlink` and `delete` use Forgejo's package API and work on any server. `inherit` and `settings` change Damast's [repo-bound packages](https://github.com/damast-dev/damast#packages-belong-to-their-repository); against plain Forgejo they stop with a clear message.

## Drop-in, like Damast

- **Same commands, same flags.** Everything in the [fj wiki](https://codeberg.org/forgejo-contrib/forgejo-cli/wiki) works with `mast` in place of `fj`.
- **Same logins.** mast reads and writes `fj`'s config, so `fj auth login` and `mast auth login` share accounts. Use both side by side, or go back to `fj` and lose nothing.
- **Works with plain Forgejo.** Everything works against any Forgejo or Damast instance. Damast-only commands will say so clearly when the server is plain Forgejo, and never get in the way of the others.

## Install

From source, until the first release:

```sh
cargo install --git https://github.com/damast-dev/mast
mast auth login
mast whoami
```

## Is mast for you?

If you use Damast, yes, eventually. Until the Damast commands land, `fj` does everything mast does and has a longer track record. Use whichever you like; they share their config.

## How the fork works

`main` is an fj release tag plus mast's commits. New fj releases arrive as `sync/vX.Y.Z` pull requests ([`upstream-sync.yml`](workflows/upstream-sync.yml)) and are merged with a merge commit, never rebased. Upstream files only get small hooks marked `DAMAST:`; mast's own code lives in its own modules. Anything `fj` would want goes to `fj` first.

## Thanks, fj

mast exists because [forgejo-cli](https://codeberg.org/forgejo-contrib/forgejo-cli) exists. If mast is useful to you, support `fj` and its maintainers too. The upstream README is in [`README.md`](../README.md).

## License

Apache-2.0 OR MIT, like forgejo-cli.
