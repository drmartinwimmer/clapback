{
  description = "Declarative CLI command builder and runner with clap-style derive macros and composition";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = toolchain;
          rustc = toolchain;
        };
        clapbackPackage = rustPlatform.buildRustPackage {
          pname = "clapback";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          installPhase = ''
            runHook preInstall
            mkdir -p $out/lib
            runHook postInstall
          '';
        };
      in
      {
        devShells.default =
          with pkgs;
          mkShell {
            buildInputs = [
              toolchain
              (cargo-audit.override { inherit rustPlatform; })
              taplo
              prettier
            ];
          };

        packages = {
          default = clapbackPackage;
          clapback = clapbackPackage;
        };
      }
    );
}
