{
  description = "wcontainers";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];

      perSystem =
        { system, pkgs, ... }:
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ inputs.rust-overlay.overlays.default ];
          };

          # Docker and Chrome are used by the tests but stay outside Nix.
          devShells.default = pkgs.mkShell (
            {
              packages = [
                (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
                pkgs.nixfmt
                pkgs.nodejs
                pkgs.taplo
                # running and inspecting WebAssembly
                pkgs.wasmtime
                pkgs.wasm-tools
                # building ARM64 Linux test programs, and the C parts of test-only dependencies
                pkgs.zig
                pkgs.cmake
                pkgs.pkg-config
                # random C programs and shrinking the ones that fail
                pkgs.csmith
                pkgs.creduce
              ];
              # driving the installed Chrome from Node, for the browser runner
              WC_PLAYWRIGHT_CORE = "${pkgs.playwright-driver}";
            }
            // pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
              # bindgen, for unicorn-engine-sys; on macOS it finds Xcode's
              LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
              # unicorn's 16-byte atomics are calls into libatomic under GCC
              "CARGO_TARGET_${pkgs.stdenv.hostPlatform.rust.cargoEnvVarTarget}_RUSTFLAGS" =
                "-C link-arg=-latomic";
            }
          );

          formatter = pkgs.nixfmt;
        };
    };
}
