{ pkgs ? import <nixpkgs> {}, lib ? pkgs.lib, ... }:

pkgs.rustPlatform.buildRustPackage {
    name = "mpv-widget";
    src = lib.cleanSource (builtins.filterSource
        (path: type: !(type == "directory" && baseNameOf path == "target"))
        ./.);
    cargoLock = {
        lockFile = ./Cargo.lock;
        outputHashes = {
        };
    };

    buildInputs = [ pkgs.mpv pkgs.fontconfig ];
    nativeBuildInputs = [ pkgs.cmake pkgs.pkg-config ];
}
