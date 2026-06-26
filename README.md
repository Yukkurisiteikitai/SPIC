# slidecli

`slidecli` is a terminal slide editor and presenter built with Rust and Ratatui.

## Install on macOS with Homebrew

```bash
brew tap Yukkurisiteikitai/spic
brew trust yukkurisiteikitai/spic
brew install slidecli
```

## Local development

```bash
cargo run
cargo test
cargo build --release
```

## Release automation

Publishing a GitHub Release for a `vX.Y.Z` tag automatically:

- builds macOS release binaries for Apple Silicon and Intel
- uploads `.tar.gz` release assets that contain `slidecli` and `LICENSE`
- updates `Formula/slidecli.rb` in `Yukkurisiteikitai/homebrew-spic`

The release tag must match `package.version` in `Cargo.toml`. The source repository also needs a `HOMEBREW_TAP_TOKEN` secret with push access to the tap repository.
