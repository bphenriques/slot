# Tools for building the card and putting it on a device. Docker comes from the host. On an
# x86_64 host the device build also needs aarch64 emulation, in configuration.nix:
#   boot.binfmt.emulatedSystems = [ "aarch64-linux" ];
#   boot.binfmt.preferStaticEmulators = true;
{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  packages = with pkgs; [
    go-task
    rustup
    curl
    unzip
    file
    rsync
    android-tools
    cargo-llvm-cov
    cargo-zigbuild
    zig
  ];
}
