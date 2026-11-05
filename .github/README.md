# mast

**The command line for [Damast](https://damast.dev).** What `gh` is to GitHub, `mast` wants to be for your own forge.

> [!NOTE]
> mast is a soft fork of [forgejo-cli](https://codeberg.org/forgejo-contrib/forgejo-cli) (`fj`), currently based on **fj v0.7.0**. Every new fj release arrives here as a pull request and ships as a mast release.

## Why mast exists

On GitHub, `gh` is half the workflow. You open pull requests from the terminal, check out someone's branch with one command, watch a workflow run, publish a release. Teams that move to a self-hosted forge miss that more than they expect.

`fj` already does a lot of this for Forgejo, and does it well. mast starts from `fj` and adds what Damast adds to Forgejo, so the command line knows about the same things the web UI does.

**Right now, mast is `fj` with a new name.** That is on purpose: it gives the fork a clean starting point that tracks `fj` from day one. Damast commands come next, starting with repo-bound packages.

## What mast adds

### Security advisories

```sh
mast security policy                                    # the repository's SECURITY.md
mast security report -t "SQL injection in search" -b -  # report a vulnerability privately (body from stdin)
mast advisory list                                      # advisories you may see; --state draft, --json
mast advisory view DAMAST-58rf-qmv9-p456                # one advisory; --osv for OSV JSON
mast advisory create -t "XSS in comments" -b "..." --cvss CVSS:3.1/... --cwe CWE-79 \
  --ecosystem npm --package app --vulnerable ">= 1.0, < 1.4.2" --patched 1.4.2
mast advisory publish DAMAST-6hqg-4vm3-pv9g
mast advisory close DAMAST-58rf-qmv9-p456
mast advisory withdraw DAMAST-6hqg-4vm3-pv9g            # published by mistake: stays visible, marked withdrawn
mast advisory comments DAMAST-58rf-qmv9-p456            # the private discussion
mast advisory comment DAMAST-58rf-qmv9-p456 -b "Fix is in review"
mast advisory comment-edit DAMAST-58rf-qmv9-p456 12 -b "Fix is merged"   # your own comments; admins any
mast advisory comment-delete DAMAST-58rf-qmv9-p456 12
mast advisory list --osv > advisories.osv.json          # export the published advisories
```

These use Damast's [security advisories](https://github.com/damast-dev/damast#security-advisories-and-private-vulnerability-reports) API. Against plain Forgejo they stop with a clear message; only `mast security policy` works everywhere, because it just reads the file.

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
