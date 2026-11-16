# Money

A simple personal finance tracker.
Bank statements (QFX/OFX and CSV) are imported from per-account directories into a local SQLite database
when the program starts, categorized by rules from a TOML config,
and shown in a native desktop GUI (egui, GPU rendered via wgpu) on Windows and Linux.

## Building and running

```bash
cargo run --release
```

The config is read from `config.toml` in the OS data directory (`~/.local/share/money_app` on
Linux, `%APPDATA%\money_app` on Windows); pass `--config <path>` to use another file.
If no config exists the app opens the settings editor so you can create one.

To try the app with generated data:

```bash
cargo run --all-features -- --mock /tmp/money-mock
```

Other options: `--summary` imports and prints a summary without opening a window, and
`--tab chart|uncategorized|settings|transactions` picks the starting tab.

Building on Linux needs the usual windowing libraries (`libxkbcommon`, Wayland and/or X11).

The UI uses the [Inter](https://rsms.me/inter/) typeface, bundled in `assets/fonts` under the
SIL Open Font License (see `assets/fonts/Inter-LICENSE.txt`). Light, dark or system theme can
be picked at the bottom of the sidebar.

## Config format

```toml
[[account]]
name = "Chequing"
source_path = "/home/me/statements/chequing"   # searched recursively for .qfx/.csv

# How raw transactions map to a transaction type, either by name prefix…
[[transaction_type]]
prefix = "POS "
transaction_type = "DebitPurchase"
name_source = "NameSuffix"    # Memo | Name | NameSuffix
accounts = ["Chequing"]

# …or by the type reported in the file.
[[transaction_type]]
mode = "SourceType"
source_type = "Credit"        # Debit | Credit | Pos | Atm | Fee | Other
transaction_type = "CreditRefund"
income = "auto"               # yes | no | auto (positive amounts are income)
name_source = "Name"
accounts = ["Visa"]

# Rules match the decoded name exactly and assign a dot separated category.
[[rule]]
transaction_type = "DebitPurchase"
category = "Food.Groceries"
patterns = ["GROCERY MART", "FRESH FOODS"]

[[rule]]
transaction_type = "CreditRefund"
category = "Transfer"
ignore = true                 # drop matching transactions
patterns = ["PAYMENT THANK YOU"]
```

Saving from the settings editor rewrites the file (comments are not preserved) and keeps the
previous version as `config.toml.bak`.

## Development

```bash
cargo test --all-targets  --all-features    # unit tests + headless GUI tests
cargo clippy --all-targets  --all-features -- -D missing_docs -D clippy::missing_docs_in_private_items
cargo llvm-cov --all-targets  --all-features --summary-only # coverage (CI requires ≥ 85% of lines)
```

Set `MONEY_SNAPSHOT_DIR=<dir>` when running `cargo test --test gui` to save PNG renderings of
each view (requires a Vulkan/Metal/DX12/GL adapter; a software one such as lavapipe works).
each view in light and dark mode (requires a Vulkan/Metal/DX12/GL adapter; a software one
such as lavapipe works).
