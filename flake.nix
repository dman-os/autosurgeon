{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { flake-parts, ... } @ inputs: flake-parts.lib.mkFlake { inherit inputs; } {
    imports = [
    ];

    perSystem = { config, self', inputs', pkgs, system,  ... }: 
      # Allows definition of system-specific attributes
      # without needing to declare the system explicitly!
      #
      # Quick rundown of the provided arguments:
      # - config is a reference to the full configuration, lazily evaluated
      # - self' is the outputs as provided here, without system. (self'.packages.default)
      # - inputs' is the input without needing to specify system (inputs'.foo.packages.bar)
      # - pkgs is an instance of nixpkgs for your specific system
      # - system is the system this configuration is for
      let 
        envVars = {
          CARGO_BUILD_JOBS = "8";
        };

        rustVersion = "1.94.0";
        rustStable = pkgs.rust-bin.stable.${rustVersion}.default.override {
          extensions = [ "rust-src" ];
          targets = [
            # "wasm32-unknown-unknown"
            # "wasm32-wasip2"
          ];
        };

        toolInputs = with pkgs; [
          prek
          rustStable
        ];

        ldPaths = with pkgs; lib.makeLibraryPath (
          lib.map (x: lib.getLib x) (
            [ 
            ]
          )
        );

        shellDev = pkgs.mkShell ({
          buildInputs = toolInputs ++ (with pkgs; [
            gh
          ]);
          shellHook = ''
            export PATH=$PATH:$PWD/x/
            export LD_LIBRARY_PATH="$$LD_LIBRARY_PATH:${ldPaths}"
            # use the users preferred shell
            exec $(getent passwd $USER | cut -d: -f7)
          '';
        } // envVars);

        shellCi = pkgs.mkShell ({
          buildInputs = toolInputs ++ (with pkgs; [
          ]);
          shellHook = ''
            export PATH=$PATH:$PWD/x/
            export LD_LIBRARY_PATH="$$LD_LIBRARY_PATH:${ldPaths}"
          '';
        } // envVars);
    in {
      _module.args.pkgs = import inputs.nixpkgs {
        inherit system;
        overlays = [
          (import inputs.rust-overlay)
        ];
        config = { };
      };

      devShells = {
        default = shellDev;
        ci = shellCi;
      };
    };

    flake = {
      # The usual flake attributes can be defined here, including
      # system-agnostic and/or arbitrary outputs.
    };


    # Declared systems that your flake supports. These will be enumerated in perSystem
    systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
  };
}
