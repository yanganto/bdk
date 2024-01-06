{
  description = "BDK Flake to run all tests locally and in CI";

  inputs = {
    # stable nixpkgs (let's not YOLO on unstable)
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";

    # pin dependencies to a specific version
    # find instructions here:
    # <https://lazamar.co.uk/nix-versions>

    # bitcoind pinned to 0.25.1
    nixpkgs-bitcoind.url = "github:nixos/nixpkgs?rev=53793ca0aecf67164c630ca34dbfde552a8ab085";
    # TODO: pin to 0.26.0 once it stops to fail in darwin
    # pinned to 0.26.0
    # nixpkgs-bitcoind.url = "github:nixos/nixpkgs?rev=f5375ec98618347da6b036a5e06381ab7380db03";

    # Blockstream's esplora
    # inspired by fedimint CI
    nixpkgs-kitman.url = "github:jkitman/nixpkgs?rev=61ccef8bc0a010a21ccdeb10a92220a47d8149ac";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };

    flake-utils.url = "github:numtide/flake-utils";

    pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";
  };

  outputs = { self, nixpkgs, nixpkgs-bitcoind, nixpkgs-kitman, rust-overlay, flake-utils, pre-commit-hooks, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        lib = pkgs.lib;
        stdenv = pkgs.stdenv;
        isDarwin = stdenv.isDarwin;
        libsDarwin = with pkgs.darwin.apple_sdk.frameworks; lib.optionals isDarwin [
          # Additional darwin specific inputs can be set here
          Security
          SystemConfiguration
          CoreServices
        ];

        # Dependencies
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        pkgs-bitcoind = import nixpkgs-bitcoind {
          inherit system overlays;
        };
        pkgs-kitman = import nixpkgs-kitman {
          inherit system;
        };

        # Signed Commits
        signed-commits = pkgs.writeShellApplication {
          name = "signed-commits";
          runtimeInputs = [ pkgs.git ];
          text = builtins.readFile ./ci/commits_verify_signature.sh;
        };

        # Toolchains
        # latest stable
        stable = pkgs.rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ]; # wasm
        };
        # MSRV stable
        msrv = pkgs.rust-bin.stable."1.63.0".default.override {
          targets = [ "wasm32-unknown-unknown" ]; # wasm
        };
        # Nighly for docs
        nightly = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default);
        # Code coverage
        coverage = pkgs.rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ]; # wasm
          extensions = [ "llvm-tools-preview" ];
        };

        # Common inputs
        envVars = {
          BITCOIND_EXEC = "${pkgs-bitcoind.bitcoind}/bin/bitcoind";
          ELECTRS_EXEC = "${pkgs-kitman.esplora}/bin/esplora";
          CC = "${stdenv.cc.nativePrefix}cc";
          AR = "${stdenv.cc.nativePrefix}ar";
          CC_wasm32_unknown_unknown = "${pkgs.llvmPackages_14.clang-unwrapped}/bin/clang-14";
          CFLAGS_wasm32_unknown_unknown = "-I ${pkgs.llvmPackages_14.libclang.lib}/lib/clang/14.0.6/include/";
          AR_wasm32_unknown_unknown = "${pkgs.llvmPackages_14.llvm}/bin/llvm-ar";
        };
        buildInputs = [
          # Add additional build inputs here
          pkgs-bitcoind.bitcoind
          pkgs-kitman.esplora
          pkgs.openssl
          pkgs.openssl.dev
          pkgs.pkg-config
          pkgs.curl
          pkgs.libiconv
        ] ++ libsDarwin;

        # WASM deps
        WASMInputs = [
          # Additional wasm specific inputs can be set here
          pkgs.llvmPackages_14.clang-unwrapped
          pkgs.llvmPackages_14.stdenv
          pkgs.llvmPackages_14.libcxxClang
          pkgs.llvmPackages_14.libcxxStdenv
        ];

        nativeBuildInputs = [
          # Add additional build inputs here
          pkgs.python3
        ] ++ lib.optionals isDarwin [
          # Additional darwin specific native inputs can be set here
        ];

        # Cargo update
        cargoUpdate = ''
          cargo update
        '';
        # MSRV compat tweaks
        msrvTweaks = ''
          cargo update -p home --precise "0.5.5"
        '';
      in
      {
        checks = {
          # Pre-commit checks
          pre-commit-check =
            let
              # this is a hack based on https://github.com/cachix/pre-commit-hooks.nix/issues/126
              # we want to use our own rust stuff from oxalica's overlay
              _rust = pkgs.rust-bin.stable.latest.default;
              rust = pkgs.buildEnv {
                name = _rust.name;
                inherit (_rust) meta;
                buildInputs = [ pkgs.makeWrapper ];
                paths = [ _rust ];
                pathsToLink = [ "/" "/bin" ];
                postBuild = ''
                  for i in $out/bin/*; do
                    wrapProgram "$i" --prefix PATH : "$out/bin"
                  done
                '';
              };
            in
            pre-commit-hooks.lib.${system}.run {
              src = ./.;
              hooks = {
                rustfmt = {
                  enable = true;
                  entry = lib.mkForce "${rust}/bin/cargo-fmt fmt --all -- --config format_code_in_doc_comments=true --check --color always";
                };
                clippy = {
                  enable = true;
                  entry = lib.mkForce "${rust}/bin/cargo-clippy clippy --all-targets --all-features -- -D warnings";
                };
                nixpkgs-fmt.enable = true;
                typos.enable = true;
                commitizen.enable = true; # conventional commits
                signedcommits = {
                  enable = true;
                  name = "signed-commits";
                  description = "Check whether the current commit message is signed";
                  stages = [ "push" ];
                  entry = "${signed-commits}/bin/signed-commits";
                  language = "system";
                  pass_filenames = false;
                };
              };
            };
        };

        devShells =
          let
            # pre-commit-checks
            _shellHook = (self.checks.${system}.pre-commit-check.shellHook or "");
          in
          {
            default = pkgs.mkShell ({
              shellHook = "${_shellHook}";
              buildInputs = buildInputs ++ WASMInputs ++ [ stable ];
              inherit nativeBuildInputs;
            } // envVars // {
              shellHook = "${cargoUpdate} ${_shellHook}";
            });

            msrv = pkgs.mkShell ({
              shellHook = "${_shellHook}";
              buildInputs = buildInputs ++ WASMInputs ++ [ msrv ];
              inherit nativeBuildInputs;
            } // envVars // {
              shellHook = "${msrvTweaks} ${_shellHook}";
            });

            nightly = pkgs.mkShell ({
              shellHook = "${_shellHook}";
              buildInputs = buildInputs ++ [ nightly ];
              inherit nativeBuildInputs;
            } // envVars // {
              shellHook = "${cargoUpdate} ${_shellHook}";
            });

            coverage = pkgs.mkShell ({
              shellHook = "${_shellHook}";
              buildInputs = buildInputs ++ [ coverage pkgs.lcov ];
              inherit nativeBuildInputs;
              RUSTFLAGS = "-Cinstrument-coverage";
              RUSTDOCFLAGS = "-Cinstrument-coverage";
              LLVM_PROFILE_FILE = "./target/coverage/%p-%m.profraw";
            } // envVars // {
              shellHook = "${cargoUpdate} ${_shellHook}";
            });
          };
      }
    );
}
