#!/usr/bin/env bash
# Adds .debs to the apt repository in <dir> (the gh-pages checkout of damast-dev/apt) and signs it.
# Every .deb ever added stays in pool/ and in the index, so older versions remain installable.
#   apt-repo.sh <dir> <deb>...
# Needs apt-ftparchive (apt-utils) and gpg whose keyring holds exactly the signing key;
# APT_GPG_PASSPHRASE is the key's passphrase (empty for an unprotected key).
set -euo pipefail
dir=$1
shift
mkdir -p "$dir/pool/main/m/mast"
cp "$@" "$dir/pool/main/m/mast/"
cd "$dir"

dist=dists/stable
for arch in amd64 arm64; do
	mkdir -p "$dist/main/binary-$arch"
	apt-ftparchive --arch "$arch" packages pool >"$dist/main/binary-$arch/Packages"
	gzip -9nkf "$dist/main/binary-$arch/Packages"
done

rm -f "$dist/Release" "$dist/InRelease" "$dist/Release.gpg" # else they hash themselves
apt-ftparchive \
	-o APT::FTPArchive::Release::Origin=Damast \
	-o APT::FTPArchive::Release::Label=mast \
	-o APT::FTPArchive::Release::Suite=stable \
	-o APT::FTPArchive::Release::Codename=stable \
	-o APT::FTPArchive::Release::Architectures="amd64 arm64" \
	-o APT::FTPArchive::Release::Components=main \
	-o APT::FTPArchive::Release::Description="mast, the command line for Damast" \
	release "$dist" >Release.tmp
mv Release.tmp "$dist/Release"

sign() { gpg --batch --yes --pinentry-mode loopback --passphrase-fd 0 "$@" <<<"${APT_GPG_PASSPHRASE:-}"; }
sign --clearsign -o "$dist/InRelease" "$dist/Release"
sign --armor --detach-sign -o "$dist/Release.gpg" "$dist/Release"

# Public key for `signed-by=`, always the key that signed this index.
gpg --export >mast-archive-keyring.gpg
gpg --armor --export >mast-archive-keyring.asc
touch .nojekyll
