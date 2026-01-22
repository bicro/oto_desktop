#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CSV_FILE="$PROJECT_ROOT/beta_testers.csv"
OUTPUT_DIR="$PROJECT_ROOT/beta_builds"
BUILD_TEMP_DIR="$PROJECT_ROOT/.beta_build_temp"

echo "=========================================="
echo "Beta Build Script"
echo "=========================================="

# Create output and temp directories
mkdir -p "$OUTPUT_DIR"
mkdir -p "$BUILD_TEMP_DIR"

# Load Apple code signing credentials
source "$PROJECT_ROOT/.env.local"
export APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID

# Inject signing identity into tauri.conf.json
TAURI_CONF="$PROJECT_ROOT/src-tauri/tauri.conf.json"
if [ -n "$APPLE_SIGNING_IDENTITY" ]; then
    jq --arg id "$APPLE_SIGNING_IDENTITY" '.bundle.macOS.signingIdentity = $id' "$TAURI_CONF" > "$TAURI_CONF.tmp" && mv "$TAURI_CONF.tmp" "$TAURI_CONF"
    echo "Injected signing identity: $APPLE_SIGNING_IDENTITY"
fi

# Install dependencies
echo "Installing dependencies..."
(cd "$PROJECT_ROOT" && bun install)
echo "Dependencies installed!"

# Create individual build scripts for each user
echo ""
echo "Preparing build jobs..."
JOB_DIR="$BUILD_TEMP_DIR/jobs"
mkdir -p "$JOB_DIR"

USER_COUNT=0
tail -n +2 "$CSV_FILE" | while IFS=',' read -r discord_id username api_key; do
    cat > "$JOB_DIR/build_$username.sh" << EOF
#!/bin/bash
set -e
cd "$PROJECT_ROOT"

# Ensure bun is in PATH
export PATH="\$HOME/.bun/bin:\$PATH"

# Load credentials
source "$PROJECT_ROOT/.env.local"
export APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID

USERNAME="$username"
API_KEY="$api_key"
USER_TARGET_DIR="$BUILD_TEMP_DIR/target_\$USERNAME"
LOG_FILE="$BUILD_TEMP_DIR/log_\$USERNAME.txt"

echo "[\$(date +%H:%M:%S)] [START] Building for: \$USERNAME"

{
    CARGO_TARGET_DIR="\$USER_TARGET_DIR" \\
    OPENROUTER_API_KEY="\$API_KEY" \\
    bunx tauri build --target universal-apple-darwin 2>&1

    DMG_SOURCE="\$USER_TARGET_DIR/universal-apple-darwin/release/bundle/dmg/Oto Desktop_0.1.0_universal.dmg"
    mkdir -p "$OUTPUT_DIR/\$USERNAME"
    cp "\$DMG_SOURCE" "$OUTPUT_DIR/\$USERNAME/"
} > "\$LOG_FILE" 2>&1

if [ \$? -eq 0 ]; then
    echo "[\$(date +%H:%M:%S)] [SUCCESS] \$USERNAME"
else
    echo "[\$(date +%H:%M:%S)] [FAILED] \$USERNAME - Check: \$LOG_FILE"
    exit 1
fi
EOF
    chmod +x "$JOB_DIR/build_$username.sh"
    USER_COUNT=$((USER_COUNT + 1))
done

# Count total users
TOTAL_USERS=$(tail -n +2 "$CSV_FILE" | wc -l | tr -d ' ')
echo "Total users to build: $TOTAL_USERS"
echo ""
echo "Starting builds..."
echo "=========================================="

# Run builds sequentially
COMPLETED=0
FAILED=0
CURRENT=0

for script in "$JOB_DIR"/build_*.sh; do
    username=$(basename "$script" | sed 's/build_//;s/.sh//')
    CURRENT=$((CURRENT + 1))

    echo "Building: $username ($CURRENT/$TOTAL_USERS)"

    if bash "$script"; then
        COMPLETED=$((COMPLETED + 1))
        echo "  -> Success (completed: $COMPLETED, failed: $FAILED)"
    else
        FAILED=$((FAILED + 1))
        echo "  -> Failed (completed: $COMPLETED, failed: $FAILED)"
    fi
done

echo ""
echo "=========================================="
echo "All builds complete!"
echo "Completed: $COMPLETED"
echo "Failed: $FAILED"
echo "Output directory: $OUTPUT_DIR"
echo ""
echo "Build logs available at: $BUILD_TEMP_DIR/log_*.txt"
echo ""
ls -la "$OUTPUT_DIR"

# Show any failed builds
if [ $FAILED -gt 0 ]; then
    echo ""
    echo "Failed builds - check these logs:"
    for log in "$BUILD_TEMP_DIR"/log_*.txt; do
        if grep -q "error\|Error\|FAILED" "$log" 2>/dev/null; then
            echo "  - $log"
        fi
    done
fi

# Restore tauri.conf.json to clean state
jq '.bundle.macOS.signingIdentity = null' "$TAURI_CONF" > "$TAURI_CONF.tmp" && mv "$TAURI_CONF.tmp" "$TAURI_CONF"
echo "Restored tauri.conf.json to clean state"

# Optionally clean up temp target directories to save disk space
echo ""
read -p "Clean up temporary build directories (~20GB+)? [y/N] " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Cleaning up..."
    rm -rf "$BUILD_TEMP_DIR"/target_*
    rm -rf "$JOB_DIR"
    echo "Cleanup complete!"
fi
