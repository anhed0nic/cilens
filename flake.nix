{
  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    naersk.url = "github:nix-community/naersk";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs =
    inputs@{ flake-parts, naersk, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      debug = false;
      systems = [
        "x86_64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
        "aarch64-linux"
      ];

      perSystem =
        { pkgs, self', ... }:
        let
          naersk' = pkgs.callPackage naersk { };
        in
        rec {
          packages.default = naersk'.buildPackage { src = ./.; };

          devShells.default = pkgs.mkShell {
            packages = [
              pkgs.rustc
              pkgs.cargo

              self'.packages.default
            ];
          };
        };
    };
}
