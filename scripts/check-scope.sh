#!/usr/bin/env bash
# scripts/check-scope.sh - Enforces normative eBPF hook allow-list per Katana PRD §4, §8.2, §27.
set -euo pipefail

BPF_DIR="${1:-bpf}"

if [ ! -d "$BPF_DIR" ]; then
    echo "check-scope: directory '$BPF_DIR' does not exist yet (skipping until BPF files exist)."
    exit 0
fi

# Allowed SEC(...) definitions in C eBPF source files
ALLOWED_SECTIONS='^(tp/sched/sched_switch|tp/sched/sched_waking|tp/sched/sched_wakeup|tp/sched/sched_wakeup_new|tp/sched/sched_process_fork|tp/sched/sched_process_exit|tp/syscalls/sys_enter_futex|tp/syscalls/sys_exit_futex|tp/syscalls/sys_enter_futex_waitv|tp/syscalls/sys_enter_futex_wait|tp/syscalls/sys_enter_futex_wake|tp/syscalls/sys_enter_futex_requeue)$'

FAIL=0

while IFS= read -r match; do
    sec=$(echo "$match" | sed -E 's/.*SEC\("([^"]+)"\).*/\1/')
    if ! echo "$sec" | grep -Eq "$ALLOWED_SECTIONS"; then
        echo "SCOPE VIOLATION: Disallowed eBPF hook SEC(\"$sec\") found in: $match" >&2
        FAIL=1
    fi
done < <(grep -rnE 'SEC\("[^"]+"\)' "$BPF_DIR" || true)

if [ "$FAIL" -ne 0 ]; then
    echo "ERROR: Scope guard check failed per PRD §4 / §8.2. Only approved tracepoints are allowed." >&2
    exit 1
fi

echo "check-scope: all eBPF hooks within normative allow-list."
exit 0
