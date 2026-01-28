# Developer Note

Just some note for developer.

## Run with logs

This will show logs on stdout:

```sh
RUST_LOG=Dome3=info cargo run
```

## Build Binary

### macOS.app

```sh
cargo bundle --release
```

### Remove App Icon from the Dock

After `Dome3.app` has been built, add this property list key into `Info.plist` to hidden app icon.

```xml
<key>LSUIElement</key>
<true/>
```

### Windows.exe

```sh
cargo build --target x86_64-pc-windows-gnu --release
```
