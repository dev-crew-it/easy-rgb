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
            rev = "0b39cf0318f04a4c3f3bb59d2c63cd8ba856a0d9";
            sha256 = "sha256-y+4i8srlaiVfNnIKhLhUUSrcdSjD2/WOjLSzffcoX88=";
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
          '';
        };
      }
    );
}
