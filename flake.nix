{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      utils,
    }:
    utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      rec {
        packages.forgejo-cli = pkgs.rustPlatform.buildRustPackage {
          pname = "forgejo-cli";
          version = "0.7.0";
          src = pkgs.lib.cleanSource ./.;

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [
            pkg-config
            installShellFiles
          ];
          buildInputs = with pkgs; [ openssl ];

          postInstall = ''
            export HOME=$(mktemp -d)
            installShellCompletion --cmd fj \
              --bash <($out/bin/fj completion bash) \
              --fish <($out/bin/fj completion fish) \
              --zsh <($out/bin/fj completion zsh) \
              --nushell <($out/bin/fj completion nushell)
          '';

          meta = with pkgs.lib; {
            description = "CLI tool for Forgejo";
            homepage = "https://codeberg.org/forgejo-contrib/forgejo-cli/";
            license = with licenses; [
              asl20 # or
              mit
            ];
          };

          env = {
            BUILD_TYPE = "flake";
          };
        };

        packages.default = packages.forgejo-cli;

        devShells.default = pkgs.mkShell {
          inputsFrom = [ packages.default ];

          nativeBuildInputs = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
          ];

          # Required for rust-analyzer to work
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustcSrc}/library";
        };
      }
    );
}
