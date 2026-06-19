{
  description = "quaver_stats — HTTP service that renders Quaver player stats cards";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        quaver_stats = pkgs.rustPlatform.buildRustPackage {
          pname = "quaver_stats";
          version = "0.1.0";

          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = path: type:
              (baseNameOf path != "target")
              && (pkgs.lib.cleanSourceFilter path type);
          };

          cargoLock.lockFile = ./Cargo.lock;

          # pkg-config + openssl for reqwest's TLS; makeWrapper for wrapProgram.
          nativeBuildInputs = [ pkgs.pkg-config pkgs.makeWrapper ];
          buildInputs = [ pkgs.openssl ];

          # The font is baked in with include_bytes!, but the background image
          # is loaded at runtime. Install the assets and point the binary at
          # them via the env var the program reads.
          postInstall = ''
            mkdir -p $out/share/quaver_stats
            cp -r assets $out/share/quaver_stats/assets
            wrapProgram $out/bin/quaver_stats \
              --set-default QUAVER_STATS_ASSETS_DIR $out/share/quaver_stats/assets
          '';

          meta = with pkgs.lib; {
            description = "HTTP service that renders Quaver player stats cards as PNG";
            homepage = "https://github.com/Young-TW/quaver_stats";
            license = licenses.gpl3Plus;
            mainProgram = "quaver_stats";
            platforms = platforms.unix;
          };
        };
      in
      {
        packages.default = quaver_stats;
        packages.quaver_stats = quaver_stats;

        apps.default = flake-utils.lib.mkApp {
          drv = quaver_stats;
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [ pkgs.pkg-config pkgs.makeWrapper ];
          buildInputs = [ pkgs.openssl pkgs.cargo pkgs.rustc pkgs.clippy pkgs.rustfmt ];
          # Run `cargo run` from the repo root; assets/ is found relative to CWD.
        };
      })
    // {
      nixosModules.default = { config, lib, pkgs, ... }:
        let
          cfg = config.services.quaver_stats;
        in
        {
          options.services.quaver_stats = {
            enable = lib.mkEnableOption "the quaver_stats player-stats card service";

            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.system}.default;
              defaultText = lib.literalExpression "quaver_stats flake package";
              description = "The quaver_stats package to run.";
            };

            openFirewall = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = "Open TCP port 3001 in the firewall.";
            };
          };

          config = lib.mkIf cfg.enable {
            networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ 3001 ];

            systemd.services.quaver_stats = {
              description = "quaver_stats player-stats card service";
              wantedBy = [ "multi-user.target" ];
              after = [ "network-online.target" ];
              wants = [ "network-online.target" ];

              # The `dirs` crate resolves the avatar cache from $XDG_CACHE_HOME;
              # point it at the systemd-managed CacheDirectory.
              environment.XDG_CACHE_HOME = "/var/cache/quaver_stats";

              serviceConfig = {
                ExecStart = lib.getExe cfg.package;
                Restart = "on-failure";
                RestartSec = 5;

                DynamicUser = true;
                CacheDirectory = "quaver_stats";

                # Hardening.
                NoNewPrivileges = true;
                ProtectSystem = "strict";
                ProtectHome = true;
                PrivateTmp = true;
                PrivateDevices = true;
                ProtectKernelTunables = true;
                ProtectKernelModules = true;
                ProtectControlGroups = true;
                RestrictAddressFamilies = [ "AF_INET" "AF_INET6" ];
                RestrictNamespaces = true;
                SystemCallArchitectures = "native";
              };
            };
          };
        };
    };
}
