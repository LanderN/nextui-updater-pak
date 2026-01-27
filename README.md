![](./doc/screenshot.png)

# NextUI OTA Updater PAK

A PAK for updating NextUI on-device (aka OTA), tested on the TrimUI Brick (but might work on the Smart Pro as well). Wifi required (obviously).

## Installation

Unzip the `nextui-updater-pak.zip` file to the root of your SD card (merge the contents).

## Controls

- **D-pad Up/Down**: Navigate between buttons
- **Button A**: Select
- **Button B**: Go Back/Exit
- **Button X**: Version Select

## Building for tg5040/tg5050 using [cross-rs](https://github.com/cross-rs/cross)

```bash
cross build --release --target=aarch64-unknown-linux-gnu
```

The compiled binary will be in `target/aarch64-unknown-linux-gnu/release/nextui-updater-rs`.

## Building release zip

```bash
scripts/create_pak.sh
```

The zip file will be in `./nextui-updater-pak.zip`.

## License

This project is open source and available under the MIT License.

## Contributing

Many improvements are possible and contributions are welcome! Please feel free to submit a Pull Request.
