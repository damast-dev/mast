#!/usr/bin/env bash
# mast release versions: v<fj>-mast.<N>[-rc.<M>]
#
#   version.sh base               forgejo-cli release (vX.Y.Z) that HEAD is built on
#   version.sh next-rc            next RC tag for HEAD
#   version.sh final <rc-tag>     final tag for an RC (vX.Y.Z-mast.N-rc.M -> vX.Y.Z-mast.N)
#   version.sh latest-rc          newest RC that has no final release yet
#   version.sh self-test
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true # bash 5 (CI); also fail inside $(...), so self-test sees what CI sees

UPSTREAM=${UPSTREAM:-https://codeberg.org/forgejo-contrib/forgejo-cli.git}

# Newest stable upstream tag whose commit is in HEAD's history (upstream tags are not mirrored to origin).
base() {
	git ls-remote --tags "$UPSTREAM" 'v*' |
		sed -nE 's#^([0-9a-f]+)\trefs/tags/(v[0-9]+\.[0-9]+\.[0-9]+)(\^\{\})?$#\2 \1#p' | # lightweight and annotated (peeled) tags
		sort -rV |
		while read -r tag sha; do
			if git merge-base --is-ancestor "$sha" HEAD 2>/dev/null; then
				echo "$tag"
				return
			fi
		done
}

# next_rc <base> < existing tags: RC M+1 of the release after the newest final one.
next_rc() {
	local base=$1 tags n m
	tags=$(cat)
	n=$({ grep -E "^${base//./\\.}-mast\.[0-9]+$" <<<"$tags" || true; } | sed -E 's/.*-mast\.//' | sort -n | tail -1)
	n=$((${n:-0} + 1))
	m=$({ grep -E "^${base//./\\.}-mast\.$n-rc\.[0-9]+$" <<<"$tags" || true; } | sed -E 's/.*-rc\.//' | sort -n | tail -1)
	echo "$base-mast.$n-rc.$((${m:-0} + 1))"
}

final() {
	[[ $1 =~ ^(v[0-9]+\.[0-9]+\.[0-9]+-mast\.[0-9]+)-rc\.[0-9]+$ ]] || { echo "not an RC tag: $1" >&2; return 1; }
	echo "${BASH_REMATCH[1]}"
}

# latest_rc < existing tags
latest_rc() {
	local tags
	tags=$(cat)
	{ grep -E -- '-mast\.[0-9]+-rc\.[0-9]+$' <<<"$tags" || true; } | sort -rV | while read -r rc; do
		grep -qxF "$(final "$rc")" <<<"$tags" || { echo "$rc"; return; }
	done
}

self_test() {
	local fail=0
	check() { if [[ $2 == "$3" ]]; then echo "ok   $1"; else echo "FAIL $1: got '$2', want '$3'"; fail=1; fi; }
	check "first rc" "$(next_rc v0.7.0 </dev/null)" "v0.7.0-mast.1-rc.1"
	check "second rc" "$(printf 'v0.7.0-mast.1-rc.1\n' | next_rc v0.7.0)" "v0.7.0-mast.1-rc.2"
	check "rc 10 after 9" "$(printf 'v0.7.0-mast.1-rc.9\nv0.7.0-mast.1-rc.2\n' | next_rc v0.7.0)" "v0.7.0-mast.1-rc.10"
	check "after final" "$(printf 'v0.7.0-mast.1-rc.1\nv0.7.0-mast.1\n' | next_rc v0.7.0)" "v0.7.0-mast.2-rc.1"
	check "new fj" "$(printf 'v0.7.0-mast.3\n' | next_rc v0.7.1)" "v0.7.1-mast.1-rc.1"
	check "dots are literal" "$(printf 'v0.7.10-mast.4\n' | next_rc v0.7.0)" "v0.7.0-mast.1-rc.1"
	check "final" "$(final v0.7.0-mast.2-rc.3)" "v0.7.0-mast.2"
	check "latest rc" "$(printf 'v0.7.0-mast.1-rc.1\nv0.7.0-mast.1\nv0.7.0-mast.2-rc.1\nv0.7.0-mast.2-rc.2\n' | latest_rc)" "v0.7.0-mast.2-rc.2"
	check "no open rc" "$(printf 'v0.7.0-mast.1-rc.1\nv0.7.0-mast.1\n' | latest_rc)" ""
	if final v0.7.0-mast.1 >/dev/null 2>&1; then echo "FAIL final rejects non-RC"; fail=1; else echo "ok   final rejects non-RC"; fi
	return $fail
}

case ${1:-} in
base) base ;;
next-rc) git tag -l 'v*-mast.*' | next_rc "$(base)" ;;
final) final "$2" ;;
latest-rc) git tag -l 'v*-mast.*' | latest_rc ;;
self-test) self_test ;;
*) sed -n '2,9p' "$0" >&2; exit 2 ;;
esac
