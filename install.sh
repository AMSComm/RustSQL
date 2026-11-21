#!/usr/bin/env bash
set -e

# ==============================================================================
# RustSQL (ruql) Universal Installer for macOS and Linux
# Repository: https://github.com/AMSComm/RustSQL
# ==============================================================================

REPO="AMSComm/RustSQL"
BIN_NAME="ruql"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m' # No Color

echo -e "${CYAN}${BOLD}=== Installing RustSQL (${BIN_NAME}) ===${NC}\n"

# 1. Detect OS
OS_RAW="$(uname -s)"
case "${OS_RAW}" in
    Darwin)
        TARGET_OS="apple-darwin"
        ;;
    Linux)
        TARGET_OS="unknown-linux-gnu"
        ;;
    *)
        echo -e "${RED}Error: Unsupported operating system: ${OS_RAW}${NC}"
        echo "Please build from source using 'cargo build --release'."
        exit 1
        ;;
esac

# 2. Detect Architecture
ARCH_RAW="$(uname -m)"
case "${ARCH_RAW}" in
    x86_64|amd64)
        TARGET_ARCH="x86_64"
        ;;
    arm64|aarch64)
        TARGET_ARCH="aarch64"
        ;;
    *)
        echo -e "${RED}Error: Unsupported architecture: ${ARCH_RAW}${NC}"
        exit 1
        ;;
esac

TARGET_TRIPLE="${TARGET_ARCH}-${TARGET_OS}"
echo -e "Detected platform: ${BOLD}${TARGET_TRIPLE}${NC}"

# 3. Check for curl or wget
if command -v curl >/dev/null 2>&1; then
    FETCH_CMD="curl -sSL"
elif command -v wget >/dev/null 2>&1; then
    FETCH_CMD="wget -qO-"
else
    echo -e "${RED}Error: Neither 'curl' nor 'wget' was found on your system.${NC}"
    exit 1
fi

# 4. Fetch latest release info from GitHub API
echo -e "Fetching latest release from ${CYAN}https://github.com/${REPO}${NC}..."
RELEASE_JSON=$($FETCH_CMD "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null || true)

if [ -z "$RELEASE_JSON" ] || echo "$RELEASE_JSON" | grep -q '"message": "Not Found"'; then
    echo -e "${YELLOW}Warning: No published GitHub releases found for ${REPO} yet.${NC}"
    echo "If you have a local build, you can install directly via:"
    echo "  cargo install --path ."
    exit 1
fi

TAG_NAME=$(echo "$RELEASE_JSON" | grep '"tag_name":' | head -n 1 | cut -d '"' -f 4)
echo -e "Latest release: ${GREEN}${BOLD}${TAG_NAME}${NC}"

# 5. Find download URL matching platform triple
DOWNLOAD_URL=$(echo "$RELEASE_JSON" | grep "browser_download_url" | grep "${TARGET_TRIPLE}.tar.gz" | head -n 1 | cut -d '"' -f 4)

if [ -z "$DOWNLOAD_URL" ]; then
    echo -e "${RED}Error: No pre-built archive found for platform ${TARGET_TRIPLE}.${NC}"
    exit 1
fi

# 6. Download and extract
TEMP_DIR=$(mktemp -d)
trap 'rm -rf "$TEMP_DIR"' EXIT

ARCHIVE_FILE="${TEMP_DIR}/${BIN_NAME}.tar.gz"
echo -e "Downloading ${CYAN}${DOWNLOAD_URL}${NC}..."
if command -v curl >/dev/null 2>&1; then
    curl -sSL -o "$ARCHIVE_FILE" "$DOWNLOAD_URL"
else
    wget -qO "$ARCHIVE_FILE" "$DOWNLOAD_URL"
fi

tar -xzf "$ARCHIVE_FILE" -C "$TEMP_DIR"

if [ ! -f "${TEMP_DIR}/${BIN_NAME}" ]; then
    # Look for binary in subdirectories if any
    FOUND_BIN=$(find "$TEMP_DIR" -type f -name "${BIN_NAME}" | head -n 1)
    if [ -n "$FOUND_BIN" ]; then
        mv "$FOUND_BIN" "${TEMP_DIR}/${BIN_NAME}"
    else
        echo -e "${RED}Error: Binary '${BIN_NAME}' not found inside downloaded archive.${NC}"
        exit 1
    fi
fi

chmod +x "${TEMP_DIR}/${BIN_NAME}"

# 7. Determine installation destination
USE_SUDO=false

if [ -n "${RUQL_INSTALL_DIR:-}" ]; then
    INSTALL_DIR="$RUQL_INSTALL_DIR"
    mkdir -p "$INSTALL_DIR"
elif [ -w "/usr/local/bin" ]; then
    INSTALL_DIR="/usr/local/bin"
elif [ "${EUID:-$(id -u)}" -eq 0 ]; then
    INSTALL_DIR="/usr/local/bin"
elif sudo -n true 2>/dev/null; then
    INSTALL_DIR="/usr/local/bin"
    USE_SUDO=true
elif [ -t 0 ] && command -v sudo >/dev/null 2>&1; then
    INSTALL_DIR="/usr/local/bin"
    USE_SUDO=true
else
    INSTALL_DIR="${HOME}/.local/bin"
    mkdir -p "$INSTALL_DIR"
fi

TARGET_BIN="${INSTALL_DIR}/${BIN_NAME}"
echo -e "Installing ${BIN_NAME} to ${BOLD}${TARGET_BIN}${NC}..."

if [ "$USE_SUDO" = true ]; then
    sudo mv "${TEMP_DIR}/${BIN_NAME}" "$TARGET_BIN"
    sudo chmod 755 "$TARGET_BIN"
    # Also create psql symlink if /usr/local/bin/psql does not conflict with existing official psql
    if [ ! -e "${INSTALL_DIR}/psql" ]; then
        sudo ln -sf "$TARGET_BIN" "${INSTALL_DIR}/psql" || true
    fi
else
    mv "${TEMP_DIR}/${BIN_NAME}" "$TARGET_BIN"
    chmod 755 "$TARGET_BIN"
    if [ ! -e "${INSTALL_DIR}/psql" ]; then
        ln -sf "$TARGET_BIN" "${INSTALL_DIR}/psql" 2>/dev/null || true
    fi
fi

# 8. Verify PATH
echo -e "\n${GREEN}${BOLD}✓ RustSQL (${BIN_NAME}) installed successfully!${NC}"

if ! command -v "$BIN_NAME" >/dev/null 2>&1; then
    echo -e "${YELLOW}Notice: ${INSTALL_DIR} is not in your current PATH.${NC}"
    echo "Add the following line to your ~/.bashrc or ~/.zshrc:"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
fi

echo -e "\nVerify installation:"
echo "  $BIN_NAME --version"
echo "  $BIN_NAME --help"
echo -e "\nTo update in the future, simply run:"
echo "  $BIN_NAME --update"
