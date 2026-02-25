#!/usr/bin/env sh
set -e

REPO="itk-ai/itk"
BIN_NAME="itk"
INSTALL_DIR="/usr/local/bin"
BASE_URL="https://github.com/${REPO}/releases/latest/download"

# ── detect OS / arch ──────────────────────────────────────────────────────────
OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
  Linux*)
    case "${ARCH}" in
      x86_64|amd64)   ASSET="itk-linux-x86_64" ;;
      aarch64|arm64)  ASSET="itk-linux-aarch64" ;;
      *)
        echo "itk: unsupported architecture: ${ARCH}" >&2
        exit 1
        ;;
    esac
    ;;
  Darwin*)
    ASSET="itk-macos-universal"
    ;;
  *)
    echo "itk: unsupported OS: ${OS}" >&2
    echo "For Windows, run in PowerShell: irm https://itk-ai.app/install.ps1 | iex" >&2
    exit 1
    ;;
esac

# ── download ──────────────────────────────────────────────────────────────────
TMP="$(mktemp)"
URL="${BASE_URL}/${ASSET}"

echo "Downloading itk from ${URL} ..."

if command -v curl >/dev/null 2>&1; then
  curl -fsSL --progress-bar "${URL}" -o "${TMP}"
elif command -v wget >/dev/null 2>&1; then
  wget -q --show-progress "${URL}" -O "${TMP}"
else
  echo "itk: curl or wget is required." >&2
  rm -f "${TMP}"
  exit 1
fi

# ── install ───────────────────────────────────────────────────────────────────
chmod +x "${TMP}"

if [ -w "${INSTALL_DIR}" ]; then
  mv "${TMP}" "${INSTALL_DIR}/${BIN_NAME}"
else
  echo "Installing to ${INSTALL_DIR} (requires sudo)..."
  sudo mv "${TMP}" "${INSTALL_DIR}/${BIN_NAME}"
fi

# ── verify ────────────────────────────────────────────────────────────────────
if command -v itk >/dev/null 2>&1; then
  VERSION="$(itk --version 2>/dev/null | head -1 || echo 'unknown')"
  echo ""
  echo "✓ ${VERSION} installed to ${INSTALL_DIR}/${BIN_NAME}"
  echo ""
  echo "Quick start:"
  echo "  Copy a stack trace → run: itk"
  echo "  Pipe a git diff:          git diff | itk --diff"
  echo "  See token savings:        itk gain"
  echo ""
else
  echo ""
  echo "itk installed to ${INSTALL_DIR}/${BIN_NAME}"
  echo "If '${INSTALL_DIR}' is not in your PATH, add it:"
  echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
  echo ""
fi
