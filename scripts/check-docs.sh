#!/usr/bin/env bash
# scripts/check-docs.sh - Enforces documentation completeness, prior-art section, and banned-terms gate (PRD §27, §29, §31).
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
README="$ROOT_DIR/README.md"
DOCS_DIR="$ROOT_DIR/docs"

FAIL=0

echo "==> Running Documentation & Banned-Terms CI Gate..."

# 1. Verify README existence & Prior Art section (PRD §29)
if [ ! -f "$README" ]; then
    echo "ERROR: README.md not found!" >&2
    exit 1
fi

if ! grep -q "Prior Art" "$README"; then
    echo "ERROR: README.md lacks mandatory 'Prior Art & Positioning' section per PRD §29." >&2
    FAIL=1
fi

if ! grep -qi "Prism" "$README" || ! grep -qi "offwaketime" "$README"; then
    echo "ERROR: README.md must reference foundational prior art (Prism, BCC offwaketime) per PRD §29." >&2
    FAIL=1
fi

# 2. Verify zero banned terms in README and documentation (PRD §27, §29)
BANNED_PATTERN='(\bnovel\b|\bfirst-ever\b|\bAI-powered\b|\bai powered\b)'

while IFS= read -r match; do
    echo "BANNED TERM VIOLATION: $match" >&2
    FAIL=1
done < <(grep -rnIE "$BANNED_PATTERN" "$README" "$DOCS_DIR" || true)

# 3. Verify internal markdown file links
while IFS= read -r match; do
    file="${match%%:*}"
    dir="$(dirname "$file")"
    link="${match#*:}"
    # Extract destination inside (...)
    target=$(echo "$link" | sed -E 's/.*\]\(([^)#]+)(#[^)]*)?\).*/\1/')
    if [[ "$target" =~ ^https?:// ]] || [[ "$target" =~ ^mailto: ]] || [[ "$target" =~ ^# ]] || [[ -z "$target" ]]; then
        continue
    fi
    if [[ "$target" =~ ^file:// ]] || [[ "$target" == /* ]]; then
        echo "PORTABILITY ERROR: Absolute or file:// path forbidden in documentation: '$target' in $file (use repository-relative paths instead)" >&2
        FAIL=1
        continue
    fi
    if [ ! -e "$dir/$target" ]; then
        echo "BROKEN LINK: $target (resolved from $dir) in $file" >&2
        FAIL=1
    fi
done < <(grep -rnIE '\[[^]]+\]\([^)]+\)' "$README" "$DOCS_DIR" || true)

if [ "$FAIL" -ne 0 ]; then
    echo "ERROR: Documentation audit failed. Fix broken links, missing prior art, or banned terms." >&2
    exit 1
fi

echo "check-docs: all documentation checks and anti-inflation language gates passed."
exit 0
