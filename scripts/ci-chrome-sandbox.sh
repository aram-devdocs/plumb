#!/usr/bin/env bash
# ci-chrome-sandbox.sh — Prepare the Chrome sandbox on Linux CI runners.
#
# GitHub-hosted ubuntu-latest runners run as root inside unprivileged
# containers. Chrome's setuid sandbox refuses to start unless the kernel
# allows unprivileged user namespaces. This script enables them via
# sysctl (and adjusts the AppArmor restriction Ubuntu 24.04+ added)
# so Chrome can use its namespace sandbox — no --no-sandbox needed.
#
# The script is idempotent: re-running it when namespaces are already
# enabled is a no-op.
#
# Usage:
#   sudo ./scripts/ci-chrome-sandbox.sh
#
# Exits non-zero with a diagnostic if:
#   - Not running on Linux.
#   - The sysctl write fails and namespaces remain disabled.

set -euo pipefail

# ── Linux gate ───────────────────────────────────────────────────────
if [ "$(uname -s)" != "Linux" ]; then
    echo "ci-chrome-sandbox.sh: not Linux ($(uname -s)), skipping." >&2
    exit 1
fi

# ── Unprivileged user namespaces ─────────────────────────────────────
USERNS_SYSCTL="kernel.unprivileged_userns_clone"
APPARMOR_USERNS="/proc/sys/kernel/apparmor_restrict_unprivileged_userns"

enable_userns() {
    # Some kernels (Debian/Ubuntu patched) expose the clone sysctl.
    if [ -f "/proc/sys/kernel/$USERNS_SYSCTL" ]; then
        current=$(cat "/proc/sys/kernel/$USERNS_SYSCTL")
        if [ "$current" = "1" ]; then
            echo "User namespaces already enabled ($USERNS_SYSCTL = 1)."
        else
            echo "Enabling $USERNS_SYSCTL ..."
            sysctl -w "${USERNS_SYSCTL}=1" >/dev/null
        fi
    fi

    # Ubuntu 24.04+ restricts unprivileged user namespaces via AppArmor.
    if [ -f "$APPARMOR_USERNS" ]; then
        current=$(cat "$APPARMOR_USERNS")
        if [ "$current" = "0" ]; then
            echo "AppArmor userns restriction already disabled."
        else
            echo "Disabling AppArmor unprivileged userns restriction ..."
            echo 0 > "$APPARMOR_USERNS"
        fi
    fi
}

enable_userns

# ── Verify ───────────────────────────────────────────────────────────
# Best-effort verification: if the clone sysctl exists, confirm it is 1.
if [ -f "/proc/sys/kernel/$USERNS_SYSCTL" ]; then
    val=$(cat "/proc/sys/kernel/$USERNS_SYSCTL")
    if [ "$val" != "1" ]; then
        echo "::error::Failed to enable unprivileged user namespaces ($USERNS_SYSCTL = $val)." >&2
        exit 1
    fi
fi

if [ -f "$APPARMOR_USERNS" ]; then
    val=$(cat "$APPARMOR_USERNS")
    if [ "$val" != "0" ]; then
        echo "::error::Failed to disable AppArmor userns restriction ($APPARMOR_USERNS = $val)." >&2
        exit 1
    fi
fi

echo "Chrome sandbox preparation complete."
