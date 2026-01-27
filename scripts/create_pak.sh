#!/bin/bash

set -euo pipefail

DIST_DIR="dist"
PAK_DIR_NAME="Updater.pak"
UPDATER_BINARY="target/aarch64-unknown-linux-gnu/release/nextui-updater-rs"
ZIP_FILE="nextui-updater-pak.zip"

rm -rf "$DIST_DIR"

for PLATFORM in tg5040 tg5050; do
    UPDATER_DIR="$DIST_DIR/Tools/$PLATFORM/$PAK_DIR_NAME"
    mkdir -p "$UPDATER_DIR"

    cp "$UPDATER_BINARY" "$UPDATER_DIR/nextui-updater"
    cp "pak.json" "$UPDATER_DIR/pak.json"

    LAUNCH_SCRIPT="$UPDATER_DIR/launch.sh"
    cat > "$LAUNCH_SCRIPT" <<EOF
#!/bin/sh

cd \$(dirname "\$0")
:> logs.txt

while : ; do

./nextui-updater 2>&1 >> logs.txt

[[ \$? -eq 5 ]] || break

done

EOF
    chmod +x "$LAUNCH_SCRIPT"
done

(cd "$DIST_DIR" && zip -r "../$ZIP_FILE" .)
for PLATFORM in tg5040 tg5050; do
    (cd "$DIST_DIR/Tools/$PLATFORM/$PAK_DIR_NAME" && zip -r "../../../../$PAK_DIR_NAME_$PLATFORM.zip" .)
done
