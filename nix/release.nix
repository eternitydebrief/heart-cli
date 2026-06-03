{ pkgs ? import <nixpkgs> {} }:

let
  rp = pkgs.pkgsCross.musl64.rustPlatform;

  mk = name: rp.buildRustPackage {
    pname = name;
    version = "0.1.0";
    src = ../${name};
    cargoLock.lockFile = ../${name}/Cargo.lock;
    RUSTFLAGS = "-C target-feature=+crt-static";
    doCheck = false;
  };
in {
  heartfetch  = mk "heartfetch";
  sandboxheart = mk "sandboxheart";
}
