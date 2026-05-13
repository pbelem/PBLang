{
  description = "Development environment for a Rust compiler with LLVM";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    # rust-overlay allows selecting specific Rust versions (stable, beta, nightly)
    rust-overlay.url = "github:oxalica/rust-overlay";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, utils }:
    utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        
        # You can choose the LLVM version here (e.g. llvmPackages_18)
        llvm = pkgs.llvmPackages_18;
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Rust with useful extensions for development
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" "rust-analyzer" "clippy" ];
            })
            
            # LLVM dependencies and build tools
            llvm.llvm
            llvm.libclang
            llvm.bintools
            pkg-config
            libffi
            ncurses
            zlib
            cmake # Often required for build scripts involving C++
          ];

          # Crucial environment variables for the Rust/LLVM ecosystem
          shellHook = ''
            export LIBCLANG_PATH="${llvm.libclang.lib}/lib"
            export LLVM_SYS_180_PREFIX="${llvm.llvm.dev}"
            
            echo "--- Rust + LLVM Environment (Nix Flake) Loaded ---"
            clang --version
            rustc --version
          '';
        };
      });
}
