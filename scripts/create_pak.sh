#!/bin/bash

set -euo pipefail

DIST_DIR="dist"
UPDATER_DIR="$DIST_DIR/Tools/tg5040/Updater.pak"
UPDATER_BINARY="target/aarch64-unknown-linux-gnu/release/nextui-updater-rs"
LAUNCH_SCRIPT="$UPDATER_DIR/launch.sh"
ZIP_FILE="nextui-updater-pak.zip"

rm -rf "$DIST_DIR"
mkdir -p "$UPDATER_DIR"

cp "$UPDATER_BINARY" "$UPDATER_DIR/nextui-updater"
cp "pak.json" "$UPDATER_DIR/pak.json"

cat > "$LAUNCH_SCRIPT" <<EOF
#!/bin/sh

cd \$(dirname "\$0")
:> logs.txt

while : ; do

./nextui-updater 2>&1 >> logs.txt

[[ \$? -eq 5 ]] || break

done

EOF

(cd "$DIST_DIR" && zip -r "../$ZIP_FILE" .)
