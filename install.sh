#!/usr/bin/env bash
set -euo pipefail

MUTED=$'\033[0;2m'
ORANGE=$'\033[38;5;214m'
GREEN=$'\033[0;32m'
RED=$'\033[0;31m'
CYAN=$'\033[0;36m'
NC=$'\033[0m'

REPO="RehanIlyas-dev/Linux-Guide"
BIN_NAME="linux-guide"
VERSION="${1:-latest}"

if [ "$VERSION" = "latest" ]; then
  DOWNLOAD_URL="https://github.com/$REPO/releases/latest/download"
else
  VERSION="${VERSION#v}"
  DOWNLOAD_URL="https://github.com/$REPO/releases/download/v$VERSION"
fi

raw_os="$(uname -s)"
case "$raw_os" in
  Linux*)  OS="linux"  ;;
  Darwin*) OS="macos"  ;;
  *)
    printf "${RED}Unsupported OS: %s${NC}\n" "$raw_os"
    exit 1
    ;;
esac

raw_arch="$(uname -m)"
case "$raw_arch" in
  x86_64|amd64) ARCH="amd64"   ;;
  aarch64|arm64) ARCH="arm64"  ;;
  *)
    printf "${RED}Unsupported architecture: %s${NC}\n" "$raw_arch"
    exit 1
    ;;
esac

URL="$DOWNLOAD_URL/$BIN_NAME-$OS-$ARCH"

printf "\n${MUTED}  linux-guide installer${NC}\n"
printf "${MUTED}  %s${NC}\n" "$URL"
printf "\n"

DOWNLOAD_OK=false
if command -v curl > /dev/null 2>&1; then
  curl -# -L -o "/tmp/$BIN_NAME" "$URL" && DOWNLOAD_OK=true
elif command -v wget > /dev/null 2>&1; then
  wget --show-progress -q -O "/tmp/$BIN_NAME" "$URL" && DOWNLOAD_OK=true
else
  printf "${RED}Error: need curl or wget${NC}\n"
  exit 1
fi

if [ "$DOWNLOAD_OK" = true ]; then
  chmod +x "/tmp/$BIN_NAME"

  DEST="${DESTDIR:-/usr/local/bin}"

  if [ -w "$DEST" ]; then
    mv "/tmp/$BIN_NAME" "$DEST/$BIN_NAME"
  else
    printf "${MUTED}Installing to %s/ (may need sudo)...${NC}\n" "$DEST"
    sudo mv "/tmp/$BIN_NAME" "$DEST/$BIN_NAME"
  fi

  printf "\n${GREEN}  \xE2\x9C\x93 linux-guide installed successfully!${NC}\n"
  printf "\n"
  printf "%s _ _                                   _     _      %s\n" "$CYAN" "$NC"
  printf "%s| (_)_ __  _   ___  __      __ _ _   _(_) __| | ___ %s\n" "$CYAN" "$NC"
  printf "%s| | | '_ \| | | \ \/ /____ / _\` | | | | |/ _\` |/ _ \%s\n" "$CYAN" "$NC"
  printf "%s| | | | | | |_| |>  <_____| (_| | |_| | | (_| |  __/%s\n" "$CYAN" "$NC"
  printf "%s|_|_|_| |_|\__,_/_/\_\     \__, |\__,_|_|\__,_|\___/ %sv%s%s\n" "$CYAN" "$NC" "$MUTED" "$VERSION"
  printf "%s                           |___/                    %s\n" "$MUTED" "$NC"
  printf "\n"
  printf "${MUTED}  Run '${NC}linux-guide${MUTED}' to start${NC}\n"
  printf "${MUTED}  Learn more: ${NC}https://github.com/$REPO\n"
  printf "\n"
else
  printf "\n${ORANGE}Binary download failed (no pre-built binary for your platform).${NC}\n"
  printf "${MUTED}Falling back to building from source via cargo...${NC}\n\n"

  if command -v cargo > /dev/null 2>&1; then
    cargo install $BIN_NAME
  else
    printf "${RED}Error: cargo not found. Install Rust via https://rustup.rs${NC}\n"
    printf "${MUTED}       or build manually:${NC}\n"
    printf "         git clone https://github.com/$REPO.git\n"
    printf "         cd Linux-Guide\n"
    printf "         cargo build --release\n"
    printf "         ./target/release/linux-guide\n"
    exit 1
  fi
fi
