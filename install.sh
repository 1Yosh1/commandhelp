#!/usr/bin/env bash
set -e

# CommandHelp (`chelp`) Universal Unix Installer
# Supports Linux (x86_64) and macOS (Apple Silicon & Intel)

REPO="1Yosh1/commandhelp"
INSTALL_DIR="${HOME}/.chelp/bin"
BINARY_NAME="chelp"

BOLD="\033[1m"
GREEN="\033[32m"
BLUE="\033[34m"
RED="\033[31m"
RESET="\033[0m"

echo -e "${BLUE}${BOLD}🚀 Installing CommandHelp (chelp)...${RESET}"

# 1. Detect Operating System and Architecture
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "$OS" in
    linux)
        case "$ARCH" in
            x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
            *) echo -e "${RED}Unsupported Linux architecture: ${ARCH}${RESET}"; exit 1 ;;
        esac
        ;;
    darwin)
        case "$ARCH" in
            arm64) TARGET="aarch64-apple-darwin" ;;
            x86_64) TARGET="x86_64-apple-darwin" ;;
            *) echo -e "${RED}Unsupported macOS architecture: ${ARCH}${RESET}"; exit 1 ;;
        esac
        ;;
    *)
        echo -e "${RED}Unsupported OS: ${OS}. On Windows, run the PowerShell installer instead:${RESET}"
        echo "  irm https://raw.githubusercontent.com/${REPO}/master/install.ps1 | iex"
        exit 1
        ;;
esac

# 2. Download Release Archive
RELEASE_URL="https://github.com/${REPO}/releases/latest/download/chelp-${TARGET}.tar.gz"
TEMP_DIR="$(mktemp -d)"
TAR_FILE="${TEMP_DIR}/chelp.tar.gz"

echo -e "Downloading ${BOLD}chelp-${TARGET}.tar.gz${RESET}..."
if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$RELEASE_URL" -o "$TAR_FILE"
elif command -v wget >/dev/null 2>&1; then
    wget -qO "$TAR_FILE" "$RELEASE_URL"
else
    echo -e "${RED}Error: curl or wget is required to download chelp.${RESET}"
    exit 1
fi

# 3. Unpack and Install Binary
mkdir -p "$INSTALL_DIR"
tar -xzf "$TAR_FILE" -C "$TEMP_DIR"
cp "${TEMP_DIR}/chelp" "${INSTALL_DIR}/${BINARY_NAME}"
chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
rm -rf "$TEMP_DIR"

echo -e "${GREEN}${BOLD}✔ Installed chelp to ${INSTALL_DIR}/${BINARY_NAME}${RESET}"

# 4. Check PATH and update shell config if needed
SHELL_NAME="$(basename "$SHELL")"
PROFILE=""

case "$SHELL_NAME" in
    zsh) PROFILE="${HOME}/.zshrc" ;;
    bash)
        if [ -f "${HOME}/.bash_profile" ]; then
            PROFILE="${HOME}/.bash_profile"
        else
            PROFILE="${HOME}/.bashrc"
        fi
        ;;
    *) PROFILE="${HOME}/.profile" ;;
esac

case ":$PATH:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
        echo "export PATH=\"${INSTALL_DIR}:\$PATH\"" >> "$PROFILE"
        echo -e "Added ${BOLD}${INSTALL_DIR}${RESET} to ${BOLD}${PROFILE}${RESET}"
        export PATH="${INSTALL_DIR}:${PATH}"
        ;;
esac

echo ""
echo -e "${GREEN}${BOLD}✨ CommandHelp is installed successfully!${RESET}"
echo -e "Run ${BOLD}chelp setup${RESET} to configure your AI provider and shell integration:"
echo ""
echo -e "  ${BLUE}${BOLD}${INSTALL_DIR}/chelp setup${RESET}"
echo ""
