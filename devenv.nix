{pkgs, ...}: {
  env.RUSTC_WRAPPER = "${pkgs.sccache}/bin/sccache";

  packages = [pkgs.git pkgs.sccache pkgs.cargo-unused-features];

  languages.nix.enable = true;
  languages.rust.enable = true;
  languages.shell.enable = true;

  pre-commit.hooks.clippy.enable = true;
}
