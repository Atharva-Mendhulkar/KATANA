#!/usr/bin/env bash
# scripts/check-scope.sh - Enforces normative eBPF hook allow-list and forbidden-helper CI gate per Katana PRD §4, §8.2, §21.2, §27.
set -euo pipefail

BPF_DIR="${1:-bpf}"
SRC_DIR="${2:-src}"

FAIL=0

if [ -d "$BPF_DIR" ]; then
    # 1. Allowed SEC(...) definitions in C eBPF source files (PRD §8.2, §32.1 Phase 2)
    ALLOWED_SECTIONS='^(\.maps|license|tp/sched/sched_switch|tp/sched/sched_waking|tp/sched/sched_wakeup|tp/sched/sched_wakeup_new|tp/sched/sched_process_fork|tp/sched/sched_process_exit|tp/syscalls/sys_enter_futex|tp/syscalls/sys_exit_futex|tp/syscalls/sys_enter_futex_waitv|tp/syscalls/sys_enter_futex_wait|tp/syscalls/sys_enter_futex_wake|tp/syscalls/sys_enter_futex_requeue|tp/block/block_rq_issue|tp/block/block_rq_complete)$'

    while IFS= read -r match; do
        sec=$(echo "$match" | sed -E 's/.*SEC\("([^"]+)"\).*/\1/')
        if ! echo "$sec" | grep -Eq "$ALLOWED_SECTIONS"; then
            echo "SCOPE VIOLATION: Disallowed eBPF hook SEC(\"$sec\") found in: $match" >&2
            FAIL=1
        fi
    done < <(grep -rnE 'SEC\("[^"]+"\)' "$BPF_DIR" || true)

    # 2. Forbidden eBPF helpers (PRD §21.2): no modification, no signals, no return overrides
    FORBIDDEN_BPF_HELPERS='(bpf_override_return|bpf_send_signal|bpf_send_signal_thread|bpf_probe_write_user|bpf_set_retval)'
    while IFS= read -r match; do
        echo "SAFETY VIOLATION: Forbidden BPF helper found: $match" >&2
        FAIL=1
    done < <(grep -rnE "$FORBIDDEN_BPF_HELPERS" "$BPF_DIR" || true)
fi

# 3. Forbidden target modification in userspace (PRD §21.2): no ptrace, no signals, no priority changes, no memory write
if [ -d "$SRC_DIR" ]; then
    FORBIDDEN_USERSPACE='(ptrace|process_vm_writev|sched_setscheduler|setpriority)'
    while IFS= read -r match; do
        echo "SAFETY VIOLATION: Forbidden target modification primitive in userspace: $match" >&2
        FAIL=1
    done < <(grep -rnE "$FORBIDDEN_USERSPACE" "$SRC_DIR" || true)
fi

if [ "$FAIL" -ne 0 ]; then
    echo "ERROR: Scope guard or safety check failed per PRD §4 / §8.2 / §21.2." >&2
    exit 1
fi

echo "check-scope: all eBPF hooks and helpers within normative safety allow-list."
exit 0
