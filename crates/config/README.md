# alerion-config

Configuration file impl/handler. It reads, by default, from `/etc/alerion/config.yml` on *nix and `%ProgramData%\Alerion/config.yml` on Windows.

### TODO, probably

- [ ] define AND DOCUMENT :!!: all the config options
- [ ] filesystem watch (crates.io/notify)
- [ ] also gonna have to write to the file
- [ ] and ensure all that works OK on Windows

### Concerns

- `serde_yaml` was just archived and is now unmaintained.
- YAML is a terrible format. Change to something more sane? If being a drop-in replacement for pterodactyl wings isn't tooooo much of a concern..
