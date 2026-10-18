{ pkgs, ... }:

let
  # Cross toolchain for type-checking the cfg(windows) code paths.
  mingw = pkgs.pkgsCross.mingwW64.stdenv.cc;
in
{
  # `cargo check --target x86_64-pc-windows-gnu` type-checks the cfg(windows)
  # code without a Windows runner. psm assembles per-architecture sources and
  # stacker compiles windows.c, so both need a Windows-targeting C toolchain
  # even though checking never links. The crate gates no code on target_env,
  # so this reaches the same source the msvc CI job compiles.
  #
  # The toolchain is referenced by path rather than listed in `packages`: its
  # setup hook would export CC/AR for every target and break host builds.
  env.CC_x86_64_pc_windows_gnu = "${mingw}/bin/x86_64-w64-mingw32-gcc";
  env.AR_x86_64_pc_windows_gnu = "${mingw}/bin/x86_64-w64-mingw32-ar";

  packages = with pkgs; [
    just
    jq

    lld

    cargo-audit
    cargo-deny
    cargo-dist
    cargo-release
    cargo-watch
  ];

  languages = {
    rust = {
      enable = true;
      channel = "stable";
      targets = [ "wasm32-unknown-unknown" "x86_64-pc-windows-gnu" ];
    };
  };
}
