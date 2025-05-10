# Macro Deck Driver

The driver for Macro-Deck.

## Installation

1. Install Rust (if you haven't already)
2. Clone the repository:

   ```bash
   git clone https://github.com/nohackjustnoobb/Macro-Deck-Driver.git
   cd Macro-Deck-Driver
   ```

3. Build and install the binary:

   ```bash
   cargo install --path .
   ```

   This will compile the project and install the macro-deck-driver binary into your Cargo bin directory (typically ~/.cargo/bin).

4. (Optional) Add Cargo bin directory to your PATH (if not already):

   ```bash
   export PATH="$HOME/.cargo/bin:$PATH"
   ```

5. (Optional) Create a shortcut alias:

   You can add this to your shell profile (e.g. ~/.bashrc, ~/.zshrc):

   ```bash
   alias mdd="macro-deck-driver"
   ```

## Usage

To start the Macro Deck Driver:

```bash
macro-deck-driver start
# or with alias
mdd start
```

To stop the Macro Deck Driver:

```bash
macro-deck-driver stop
# or with alias
mdd stop
```

For more options:

```bash
macro-deck-driver help
# or with alias
mdd help
```
