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

    fenix = {
      url = "github:nix-community/fenix/monthly"; # we don't want to update the nightly toolchain every day
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        #flake-utils.follows = "flake-utils";
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
    
    flake = {
      nixosModules = rec {
        default = nar-alike-deduper;
        nar-alike-deduper = { config, lib, pkgs, ... }: with lib; let 
          cfg = config.nar-alike-deduper;
        in {
          options = {
            nar-alike-deduper = {
              enable = mkEnableOption "nar-alike-deduper";
              #port = mkOption {
              #  type = types.int;
              #  default = 8080;
              #  description = "The port to listen on";
              #};
            };
          };
          config = mkIf cfg.enable {
            nix.settings.extra-substituters = [
              #"http://localhost:4488"
            ];

            nix.settings.substituters = [
              "http://localhost:4489"
            ];


            users.users.nar-alike-deduper = {
              isSystemUser = true;
              group = "nar-alike-deduper";
              createHome = true;
              home = "/var/lib/nar-alike-deduper";
              #shell = pkgs.bashInteractive;
            };
            users.groups.nar-alike-deduper = {};
      
      
            systemd.services."nar-alike-deduper" = {
              description = "Dedups similar NARs";
              wantedBy = ["multi-user.target"];
              after = [ "network.target" "network-online.target"];
      
              serviceConfig = {
                ExecStart = "${inputs.self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/substituer"; # --port ${toString cfg.port}";
                Restart = "always";
                RestartSec = "5";
                User = "nar-alike-deduper";
                Group = "nar-alike-deduper";
                WorkingDirectory = config.users.users.nar-alike-deduper.home;
              };
            };
      
          };
        };
      };
    };


    perSystem = {pkgs, system, inputs', ...}: let
      rustToolchain = (pkgs.extend inputs.rust-overlay.overlays.default).rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override { extensions = ["rustc-codegen-cranelift-preview"]; });
      #rustToolchain = inputs'.fenix.packages.complete.withComponents ["rustc" "cargo" "rustfmt" "rust-std" "rust-docs" "rust-analyzer" "clippy" "miri" "rust-src" "rustc-codegen-cranelift-preview"];
      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;
    in {
      packages = rec {
        inherit rustToolchain;
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
        languages.rust = {
          enable = true;
          toolchain = {
            rustc = rustToolchain;
            cargo = rustToolchain;
            clippy = rustToolchain;
            rustfmt = rustToolchain;
            rust-analyzer = rustToolchain;
          };
        };
        env.RUSTFLAGS="-Zcodegen-backend=cranelift -C linker=${pkgs.clang_17}/bin/clang -C link-arg=-fuse-ld=${pkgs.mold}/bin/mold";
        packages = with pkgs; [
          #rustToolchain
          xh
        ];
      };
    };
  };
}