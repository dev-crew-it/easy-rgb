{
  description = "Integration of RGB protocol on core lightning";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlay ];
        };

        clightning = pkgs.clightning.overrideAttrs (oldAttrs: {
          version = "rgb-hooks";
          src = pkgs.fetchgit {
            url = "https://github.com/vincenzopalazzo/lightning";
            rev = "e1da6c799f302ea0346e352a07fbc083e4b7d0df";
            sha256 = "sha256-zaTNd0KnokiQdeMG9cE6Yx5FbQBA4F3Lm2vvWiWWjR8=";
            fetchSubmodules = true;
          };
          configureFlags = [ "--disable-rust" "--disable-valgrind" ];
        });
      in
      {
        packages = {
          default = pkgs.gnumake;
        };
        formatter = pkgs.nixpkgs-fmt;

        devShell = pkgs.mkShell {
          buildInputs = with pkgs; [
            # build dependencies
            libcap
            gcc
            pkg-config
            openssl
            git

            gnumake

            rustc
            cargo

            # integration test dependencies
            clightning
            bitcoind
          ];

          shellHook = ''
            export HOST_CC=gcc
            export PWD="$(pwd)"
            export RUST_LOG=info

            export PLUGIN_NAME=rgb-cln

            make
          '';
        };
      }
    );
}
