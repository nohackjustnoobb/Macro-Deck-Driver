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

## Configuration

The Macro Deck Driver needs to be configured using a `config.json` file. You can generate this configuration file using the [Config Generator](https://nohackjustnoobb.github.io/Macro-Deck-Driver/).

<details>
<summary>Advanced Configuration</summary>

You can customize the `config.json` file further for advanced use cases. Below is an example of an advanced configuration:

```jsonc
{
  "buttons": {
    "/default/0": {
      "command": "open", // Optional command to execute
      "args": ["/Applications/Discord.app"], // Optional arguments for the command
      "icon": "...." // Optional Base64 encoded image
    },
    // Nested folder support
    "/default/0/0": {
      "command": null,
      "args": null,
      "icon": "...." // Optional Base64 encoded image
    }
  },
  "status": {
    "command": "status-handler", // Optional command to start the status handler
    "args": null // Optional arguments for the status command
  }
}
```

</details>

## Usage

To start the Macro Deck Driver with the default configuration (in the same directory as `config.json`):

```bash
macro-deck-driver start
# or with alias
mdd start
```

To specify a custom configuration file path:

```bash
macro-deck-driver start -c /path/to/config.json
# or with alias
mdd start -c /path/to/config.json
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

## Status Handler

The status handler connects to the driver via TCP and facilitates communication between the client and the driver. Below are the supported messages:

### Messages Sent by the Status Handler

1. **setStatusHandler**

   This message requests the driver to set the current client as the status handler.

   ```json
   {
     "type": "setStatusHandler"
   }
   ```

2. **setStatus**

_Avoid spamming updates; the bandwidth is limited._

This message updates the device's status bar with a new image.

```jsonc
{
  "type": "setStatus",
  "value": "..." // Base64-encoded image
}
```

### Messages Sent by the Driver

1. **setStatusHandler**

   This is the driver's response to the `setStatusHandler` message, providing the resolution of the status bar.

   ```json
   {
     "type": "setStatusHandler",
     "value": [1920, 1080] // Resolution of the status bar
   }
   ```

2. **statusClicked**

   This message is sent by the driver when the status bar is clicked, providing the x-coordinate of the click position.

   ```json
   {
     "type": "statusClicked",
     "value": 123 // x-coordinate of the click position
   }
   ```
