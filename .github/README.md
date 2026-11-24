# mast

**The command line for [Damast](https://damast.dev).** What `gh` is to GitHub, `mast` wants to be for your own forge.

> [!NOTE]
> mast is a soft fork of [forgejo-cli](https://codeberg.org/forgejo-contrib/forgejo-cli) (`fj`), currently based on **fj v0.7.0**. Every new fj release arrives here as a pull request and ships as a mast release.

## Why mast exists

On GitHub, `gh` is half the workflow. You open pull requests from the terminal, check out someone's branch with one command, watch a workflow run, publish a release. Teams that move to a self-hosted forge miss that more than they expect.

`fj` already does a lot of this for Forgejo, and does it well. mast starts from `fj` and adds what Damast adds to Forgejo, so the command line knows about the same things the web UI does.

**Right now, mast is `fj` with a new name.** That is on purpose: it gives the fork a clean starting point that tracks `fj` from day one. Damast commands come next, starting with repo-bound packages.

## What mast adds

### Stacked pull requests

```sh
mast pr stack                     # the stack of the PR for the current branch, as a tree
git switch -c ui && git commit    # build the next layer on top of the current one
mast pr stack push                # push it and open a PR on top of the layer it builds on
mast pr stack sync                # after a fix or squash merge lower down: rebase every layer onto the one below
mast pr stack merge --up-to 42    # merge the bottom layers up to #42, one after another
```

Like `gh stack`: split a big change into small pull requests that build on each other. A pull request is a layer when its base branch is the head branch of another open pull request in the same repository, so there is nothing to set up. `push` finds the layer to build on by itself (the nearest open pull request whose head commit, or local branch, your branch contains), or takes `--on <pr>`. `sync` and `merge` run on the server, through Damast's normal update and merge checks: they stop at the first conflict, moved branch or layer that isn't ready, and say which. `mast pr stack` and `push` work against plain Forgejo too; `sync` and `merge` say that they need Damast.

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
