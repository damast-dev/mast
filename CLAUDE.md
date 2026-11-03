# mast

Soft fork of forgejo-cli / fj (upstream: https://codeberg.org/forgejo-contrib/forgejo-cli), the CLI for Damast.
Must stay a drop-in replacement for fj. Docs and tasks live in Jira project **MAST** (epic MAST-5).

## Hard rules

1. **Stay merge-friendly.** Own code goes in own modules (`src/damast/...`). Upstream files get only minimal hooks
   (a few lines), each marked `// DAMAST:` (TOML: `# DAMAST:`). Never reformat or reorder upstream code.
2. **Keep fj's config and key files** (paths, formats). mast and fj must be able to share logins.
3. Damast-only commands must fail with a clear message against plain Forgejo, never break other commands.
4. Code, comments and commit messages in English. Commits end with
   `Co-authored-by: Claude <noreply@anthropic.com>`.
5. Existing tests stay green; new commands come with tests.

## Upstream sync

`main` = an fj release tag + mast commits. New releases arrive as `sync/vX.Y.Z` PRs; merge them with a
**merge commit**, never squash or rebase. Releases are tagged `vX.Y.Z-mast.N`.
