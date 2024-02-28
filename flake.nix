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
            rev = "2dcdd1722be37b36a879023a9ddf074ccdce8187";
            sha256 = "sha256-IyBuY/30f6CVVtHRsdVWPKBVDUydpmEhZJ8rAJ2iP2Q=";
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
