# slidecli

`slidecli` is a terminal slide editor and presenter built with Rust and Ratatui.

Quick reference: [docs/cheatsheet.md](/Users/yuuto/learn_lab/slidecli-src/docs/cheatsheet.md:1)

## Current status

`slidecli` is still a prototype, but it can now open and save Markdown files:

- `cargo run` opens the built-in demo presentation
- `cargo run -- presentation.md` opens an existing Markdown file
- `slidecli edit presentation.md` does the same explicitly
- `Ctrl+S` saves back to the current file
- `:write other.md` saves to a new path

The Markdown support is intentionally narrow for now. Normal headings, paragraphs, slide breaks (`---`), and fenced code blocks work, while `slidecli`-specific block metadata is stored in HTML comments.

For staged reveals during presentation mode, wrap the hidden range inside a text block:

```md
<!-- slidecli:next:start -->
...
<!-- slidecli:next:end -->
```

The wrapped range stays hidden until the next reveal step in presentation mode.

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

## Reveal example

````md
# はじめに
スライド発表で一番面倒なのは何か?
<!-- slidecli:next:start -->
> デザインである
<!-- slidecli:next:end -->
````

In presentation mode, `Space`, `Enter`, or `l` first reveals the hidden part on the current slide. After all reveal markers on that slide are consumed, the same keys move to the next slide.

## Intended Markdown direction

The most practical next step is to keep normal slide content as plain Markdown and use HTML comments only for metadata that Markdown cannot represent cleanly.

For example:

````md
# Title

Intro text

<!-- slidecli:block type=exec lang=bash sig=sig:ed25519:abc123 -->
```bash
cargo test --release
```

<!-- slidecli:block type=output -->

---

## Next slide
````

That would allow:

- existing Markdown files to stay mostly readable
- `slidecli`-specific metadata to survive round trips
- future import/export support without inventing a fully custom file format

## Release automation

Publishing a GitHub Release for a `vX.Y.Z` tag automatically:

- builds macOS release binaries for Apple Silicon and Intel
- uploads `.tar.gz` release assets that contain `slidecli` and `LICENSE`
- updates `Formula/slidecli.rb` in `Yukkurisiteikitai/homebrew-spic`

The release tag must match `package.version` in `Cargo.toml`. The source repository also needs a `HOMEBREW_TAP_TOKEN` secret with push access to the tap repository.
