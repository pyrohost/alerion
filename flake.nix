{
  description = "DevShell for Alerion";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        darwinPkgs = nixpkgs.lib.optionals pkgs.stdenv.isDarwin (with pkgs.darwin; [
          apple_sdk.frameworks.AppKit
          apple_sdk.frameworks.Carbon
          apple_sdk.frameworks.Cocoa
          apple_sdk.frameworks.CoreFoundation
          apple_sdk.frameworks.IOKit
          apple_sdk.frameworks.WebKit
          apple_sdk.frameworks.Security
          apple_sdk.frameworks.DisplayServices
        ]);
      in
        with pkgs; {
          devShells = {
            default = mkShell {
              buildInputs =
                [
                  bacon
                  openssl
                  pkg-config
                  (rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
                  (rust-bin.nightly."2024-04-19".rustfmt)
                ]
                ++ darwinPkgs;
            };

            nightly = mkShell {
              buildInputs =
                [
                  bacon
                  openssl
                  pkg-config
                  (rust-bin.nightly."2024-04-19".default)
                ]
                ++ darwinPkgs;
            };
          };
        }
    );
}
