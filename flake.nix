{
  description = "Don't redownload pkgs that only differ by store hashes";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs = {
        nixpkgs-lib.follows = "nixpkgs"; 
      };
    };
    devenv = {
      url = "github:cachix/devenv";
      inputs = {
        nixpkgs.follows = "nixpkgs"; # don't override so that the cache can be used
        #flake-compat.follows = "flake-compat";
        #nix.follows = "nix"; # don't override so that the cache can be used
        #pre-commit-hooks.follows = "pre-commit-hooks-nix";
      };
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        #flake-utils.follows = "flake-utils";
      };
    };
    crane = { # eventually, use dream2nix when it's more stable
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = inputs @ {flake-parts, nixpkgs, devenv, ...}: flake-parts.lib.mkFlake { inherit inputs; } {
    imports = [
      devenv.flakeModule
    ];

    systems = nixpkgs.lib.systems.flakeExposed;

    perSystem = {pkgs, system, ...}: let
      #rustToolchain = (inputs.rust-overlay.overlays.default pkgs pkgs).rust-bin.stable.latest.default;
      rustToolchain = inputs.rust-overlay.packages.${system}.rust;
      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;
    in {
      packages = rec {
        nar-alike-deduper = pkgs.callPackage (
          {pkgs, ...}: let
          in craneLib.buildPackage {
            src = craneLib.cleanCargoSource (craneLib.path ./.);
            buildInputs = with pkgs; [
            ];
          }
        ) {};
        default = nar-alike-deduper;
      };

      devenv.shells.default = {
        languages.nix.enable = true;
        packages = with pkgs; [
          rustToolchain
          xh
        ];
      };
    };
  };
}