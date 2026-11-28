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
        let
          # The verifier Verus, run on the Rust toolchain of its release (its rust-toolchain.toml) instead of through rustup.
          verusRelease = "0.2026.11.22.1ce6a59";
          verusArchive =
            {
              aarch64-darwin = {
                platform = "arm64-macos";
                hash = "sha256-g2uMxH7RmlltTSkzeJ3DWyR6jNmTBTvMvm/Enxh5hLQ=";
              };
              aarch64-linux = {
                platform = "arm64-linux";
                hash = "sha256-//P7zs/N52q3KgidcdvGXo7rW71VXNufbL/4B2lOGTs=";
              };
              x86_64-linux = {
                platform = "x86-linux";
                hash = "sha256-y3E6cSjVg2Yw34KjQWZ1lEPgIHDErPEthzgPerGRzwg=";
              };
            }
            .${system};
          verusFiles = pkgs.fetchzip {
            url = "https://github.com/verus-lang/verus/releases/download/release%2F${verusRelease}/verus-${verusRelease}-${verusArchive.platform}.zip";
            inherit (verusArchive) hash;
          };
          verus = pkgs.runCommand "verus-${verusRelease}" { nativeBuildInputs = [ pkgs.makeWrapper ]; } ''
            for program in verus cargo-verus; do
              makeWrapper ${verusFiles}/$program $out/bin/$program --set VERUS_USE_RUSTUP 0 \
                --prefix ${
                  if pkgs.stdenv.hostPlatform.isDarwin then "DYLD_FALLBACK_LIBRARY_PATH" else "LD_LIBRARY_PATH"
                } : ${pkgs.rust-bin.stable."1.100.0".minimal}/lib
            done
          '';
          # Verus's formatter, which `cargo reviewable format` runs on the inside of every `verus! { }` block.
          # Built as `cargo install verusfmt --locked` builds it, without the self-updater (`--update`).
          verusfmt = pkgs.rustPlatform.buildRustPackage rec {
            pname = "verusfmt";
            version = "0.7.4";
            src = pkgs.fetchCrate {
              inherit pname version;
              hash = "sha256-Exq6giEM4ixc9+mUZ32EaFR2/sAnyYcqswkN+nj7gRI=";
            };
            cargoHash = "sha256-10NzNvo2exwyx0SeO6wjRT504WE92jHM7vYBoJuUK0U=";
            buildNoDefaultFeatures = true;
            # The published crate leaves out the tests and their helper crate.
            doCheck = false;
          };
        in
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
                # proving code correct
                verus
                verusfmt
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
