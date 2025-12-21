{
  description = "Terminal multiplexer for AI coding agents";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, crane, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        # Rust 2024 edition requires Rust 1.85+
        rustToolchain = pkgs.rust-bin.stable.latest.default;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        src = craneLib.cleanCargoSource ./.;

        commonArgs = {
          inherit src;
          strictDeps = true;
          pname = "amux";
          version = "0.3.0";

          buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
            pkgs.darwin.apple_sdk.frameworks.AppKit
          ];

          nativeBuildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        amux = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          
          meta = {
            description = "Terminal multiplexer for AI coding agents";
            homepage = "https://github.com/raphaelgruber/amux";
            license = pkgs.lib.licenses.mit;
            mainProgram = "amux";
          };
        });
      in
      {
        checks = {
          inherit amux;
          amux-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });
          amux-fmt = craneLib.cargoFmt { inherit src; };
        };

        packages = {
          default = amux;
          amux = amux;
        };

        apps.default = flake-utils.lib.mkApp { drv = amux; };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages = with pkgs; [
            rust-analyzer
            rustToolchain
          ];
        };
      }
    );
}
