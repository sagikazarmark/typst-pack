{ pkgs, ... }:

{
  dagger.enable = true;
  env.DAGGER_X_RELEASE = "v1.0.0-beta.10";

  # `cargo check --target x86_64-pc-windows-gnu` type-checks the cfg(windows)
  # code without a Windows runner. psm assembles per-architecture sources and
  # stacker compiles windows.c, so both need a Windows-targeting C toolchain
  # even though checking never links. The crate gates no code on target_env,
  # so this reaches the same source the msvc CI job compiles.
  env.CC_x86_64_pc_windows_gnu = "x86_64-w64-mingw32-gcc";
  env.AR_x86_64_pc_windows_gnu = "x86_64-w64-mingw32-ar";

  packages = with pkgs; [
    lld

    # Cross toolchain for type-checking the cfg(windows) code paths.
    pkgsCross.mingwW64.stdenv.cc

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
