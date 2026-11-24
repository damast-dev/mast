# mast

**The command line for [Damast](https://damast.dev).** What `gh` is to GitHub, `mast` wants to be for your own forge.

> [!NOTE]
> mast is a soft fork of [forgejo-cli](https://codeberg.org/forgejo-contrib/forgejo-cli) (`fj`), currently based on **fj v0.7.0**. Every new fj release arrives here as a pull request and ships as a mast release.

## Why mast exists

On GitHub, `gh` is half the workflow. You open pull requests from the terminal, check out someone's branch with one command, watch a workflow run, publish a release. Teams that move to a self-hosted forge miss that more than they expect.

`fj` already does a lot of this for Forgejo, and does it well. mast starts from `fj` and adds what Damast adds to Forgejo, so the command line knows about the same things the web UI does.

mast started as `fj` with a new name, so the fork tracks `fj` from day one. Damast commands land on top, one feature at a time.

## What mast adds

### Co-authors

`mast pr view <pr> commits` lists every `Co-authored-by:` trailer as a `Co-author:` line under the author, and `--oneline` ends each line with "Jonas Berg and 2 others", like Damast's web UI. mast reads the trailers itself, so this works against plain Forgejo too.

### Draft pull requests

```sh
mast pr create --draft "Rework the login page"   # opens it as a draft
mast pr ready                                     # the PR for the current branch is ready for review
mast pr draft 42                                  # back to draft
```

A draft is a pull request whose title starts with `WIP:`, which is how both Damast and Forgejo store drafts. So these commands work against plain Forgejo too, and `fj` users see the same drafts. On Damast, marking a draft ready also asks the code owners for review. mast writes `WIP:` and recognises `WIP:` and `[WIP]`. If your server is configured with other `WORK_IN_PROGRESS_PREFIXES`, `mast pr ready` and `mast pr draft` stop with a message and leave the title as it was.

### Stacked pull requests

```sh
mast pr stack                     # the stack of the PR for the current branch, as a tree
git switch -c ui && git commit    # build the next layer on top of the current one
mast pr stack push                # push it and open a PR on top of the layer it builds on
mast pr stack sync                # after a fix or squash merge lower down: rebase every layer onto the one below
mast pr stack merge --up-to 42    # merge the bottom layers up to #42, one after another
```

Like `gh stack`: split a big change into small pull requests that build on each other. A pull request is a layer when its base branch is the head branch of another open pull request in the same repository, so there is nothing to set up. `push` finds the layer to build on by itself (the nearest open pull request whose head commit, or local branch, your branch contains), or takes `--on <pr>`. `sync` and `merge` run on the server, through Damast's normal update and merge checks: they stop at the first conflict, moved branch or layer that isn't ready, and say which. `mast pr stack` and `push` work against plain Forgejo too; `sync` and `merge` say that they need Damast.

### Suggested changes from reviews

```sh
mast pr suggestions 42                        # list the suggestions in a pull request's reviews
mast pr suggestions apply -p 42 1817 1820     # commit two of them as one commit
mast pr suggestions apply --all -m "Polish"   # everything open, on the PR for the current branch
```

Reviewers write ` ```suggestion ` blocks on Damast like on GitHub. `apply` commits them to the pull request's branch the same way the web UI does: one commit, reviewers credited with `Co-authored-by:` trailers, conversations resolved, outdated or overlapping suggestions refused. Against plain Forgejo, both commands say that they need Damast.

### Security advisories

```sh
mast security policy                                    # the repository's SECURITY.md
mast security report -t "SQL injection in search" -b -  # report a vulnerability privately (body from stdin)
mast security reporting on                              # admins: accept private reports (off, or no argument to check)
mast advisory list                                      # advisories you may see; --state draft, --json
mast advisory view DAMAST-58rf-qmv9-p456                # one advisory; --osv for OSV JSON
mast advisory create -t "XSS in comments" -b "..." --cvss CVSS:3.1/... --cwe CWE-79 \
  --ecosystem npm --package app --vulnerable ">= 1.0, < 1.4.2" --patched 1.4.2
mast advisory accept DAMAST-58rf-qmv9-p456             # a private report becomes a draft advisory
mast advisory publish DAMAST-6hqg-4vm3-pv9g
mast advisory close DAMAST-58rf-qmv9-p456
mast advisory reopen DAMAST-58rf-qmv9-p456             # a closed advisory or report becomes a draft again
mast advisory withdraw DAMAST-6hqg-4vm3-pv9g            # published by mistake: stays visible, marked withdrawn
mast advisory comments DAMAST-58rf-qmv9-p456            # the private discussion
mast advisory comment DAMAST-58rf-qmv9-p456 -b "Fix is in review"
mast advisory comment-edit DAMAST-58rf-qmv9-p456 12 -b "Fix is merged"   # your own comments; admins any
mast advisory comment-delete DAMAST-58rf-qmv9-p456 12
mast advisory list --osv > advisories.osv.json          # export the published advisories
```

These use Damast's [security advisories](https://github.com/damast-dev/damast#security-advisories-and-private-vulnerability-reports) API. Against plain Forgejo they stop with a clear message; only `mast security policy` works everywhere, because it just reads the file.

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

Every release ships ready-made packages, so you don't need a Rust toolchain. mast installs as `mast` and never touches `fj`, so you can keep both.

**macOS and Linux, with Homebrew:**

```sh
brew trust --tap damast-dev/tap   # Homebrew only loads formulae from taps you trust
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
