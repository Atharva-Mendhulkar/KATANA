// bpf/katana.bpf.c - Katana Kernel Tracing eBPF Programs (PRD §8, §9, §10, §32)

#include "katana.h"

// BPF helper definitions
#define BPF_MAP_TYPE_HASH 1
#define BPF_MAP_TYPE_PERCPU_ARRAY 6
#define BPF_MAP_TYPE_RINGBUF 27

#define SEC(name) __attribute__((section(name), used))

// Forward declaration of helpers
static void *(*bpf_map_lookup_elem)(void *map, const void *key) = (void *) 1;
static long (*bpf_map_update_elem)(void *map, const void *key, const void *value, u64 flags) = (void *) 2;
static long (*bpf_map_delete_elem)(void *map, const void *key) = (void *) 3;
static u64 (*bpf_ktime_get_ns)(void) = (void *) 5;
static u64 (*bpf_get_current_pid_tgid)(void) = (void *) 14;
static u32 (*bpf_get_smp_processor_id)(void) = (void *) 8;
static void *(*bpf_ringbuf_reserve)(void *ringbuf, u64 size, u64 flags) = (void *) 131;
static void (*bpf_ringbuf_submit)(void *data, u64 flags) = (void *) 132;
static void (*bpf_ringbuf_discard)(void *data, u64 flags) = (void *) 133;
static long (*bpf_probe_read_user)(void *dst, u32 size, const void *unsafe_ptr) = (void *) 112;

// BPF Maps (PRD §8.3)
struct {
    __attribute__((aligned(8))) int type;
    int max_entries;
    int key_size;
    int value_size;
} tracked SEC(".maps") = {
    .type = BPF_MAP_TYPE_HASH,
    .max_entries = 1024,
    .key_size = sizeof(u32),
    .value_size = sizeof(struct kt_tracked),
};

struct {
    __attribute__((aligned(8))) int type;
    int max_entries;
    int key_size;
    int value_size;
} seq SEC(".maps") = {
    .type = BPF_MAP_TYPE_PERCPU_ARRAY,
    .max_entries = 1,
    .key_size = sizeof(u32),
    .value_size = sizeof(u64),
};

struct {
    __attribute__((aligned(8))) int type;
    int max_entries;
    int key_size;
    int value_size;
} events SEC(".maps") = {
    .type = BPF_MAP_TYPE_RINGBUF,
    .max_entries = 8 * 1024 * 1024, // 8 MiB (PRD §8.1)
    .key_size = 0,
    .value_size = 0,
};

static __inline u64 next_seq(void) {
    u32 zero = 0;
    u64 *val = bpf_map_lookup_elem(&seq, &zero);
    if (!val) return 0;
    *val += 1;
    return *val;
}

static __inline int is_tracked(u32 tid) {
    return bpf_map_lookup_elem(&tracked, &tid) != 0;
}

// 1. sched:sched_switch
SEC("tp/sched/sched_switch")
int handle_sched_switch(void *ctx) {
    u64 ptid = bpf_get_current_pid_tgid();
    u32 tid = (u32)ptid;
    if (!is_tracked(tid)) return 0;

    u64 s = next_seq();
    void *buf = bpf_ringbuf_reserve(&events, sizeof(struct kt_hdr) + sizeof(struct kt_switch), 0);
    if (!buf) return 0;

    struct kt_hdr *hdr = (struct kt_hdr *)buf;
    hdr->ts_ns = bpf_ktime_get_ns();
    hdr->seq = s;
    hdr->tid = tid;
    hdr->tgid = (u32)(ptid >> 32);
    hdr->cpu = (u16)bpf_get_smp_processor_id();
    hdr->type = KT_TYPE_SWITCH;
    hdr->flags = 0;

    bpf_ringbuf_submit(buf, 0);
    return 0;
}

// 2. sched:sched_waking
SEC("tp/sched/sched_waking")
int handle_sched_waking(void *ctx) {
    u64 ptid = bpf_get_current_pid_tgid();
    u32 tid = (u32)ptid;
    if (!is_tracked(tid)) return 0;

    u64 s = next_seq();
    void *buf = bpf_ringbuf_reserve(&events, sizeof(struct kt_hdr) + sizeof(struct kt_wake), 0);
    if (!buf) return 0;

    struct kt_hdr *hdr = (struct kt_hdr *)buf;
    hdr->ts_ns = bpf_ktime_get_ns();
    hdr->seq = s;
    hdr->tid = tid;
    hdr->tgid = (u32)(ptid >> 32);
    hdr->cpu = (u16)bpf_get_smp_processor_id();
    hdr->type = KT_TYPE_WAKING;
    hdr->flags = 0;

    bpf_ringbuf_submit(buf, 0);
    return 0;
}

// 3. sched:sched_wakeup
SEC("tp/sched/sched_wakeup")
int handle_sched_wakeup(void *ctx) {
    return 0;
}

// 4. sched:sched_wakeup_new
SEC("tp/sched/sched_wakeup_new")
int handle_sched_wakeup_new(void *ctx) {
    return 0;
}

// 5. sched:sched_process_fork
SEC("tp/sched/sched_process_fork")
int handle_sched_process_fork(void *ctx) {
    return 0;
}

// 6. sched:sched_process_exit
SEC("tp/sched/sched_process_exit")
int handle_sched_process_exit(void *ctx) {
    u32 tid = (u32)bpf_get_current_pid_tgid();
    if (is_tracked(tid)) {
        bpf_map_delete_elem(&tracked, &tid);
    }
    return 0;
}

// 7. syscalls:sys_enter_futex
SEC("tp/syscalls/sys_enter_futex")
int handle_sys_enter_futex(void *ctx) {
    u64 ptid = bpf_get_current_pid_tgid();
    u32 tid = (u32)ptid;
    if (!is_tracked(tid)) return 0;

    u64 s = next_seq();
    void *buf = bpf_ringbuf_reserve(&events, sizeof(struct kt_hdr) + sizeof(struct kt_futex_enter), 0);
    if (!buf) return 0;

    struct kt_hdr *hdr = (struct kt_hdr *)buf;
    hdr->ts_ns = bpf_ktime_get_ns();
    hdr->seq = s;
    hdr->tid = tid;
    hdr->tgid = (u32)(ptid >> 32);
    hdr->cpu = (u16)bpf_get_smp_processor_id();
    hdr->type = KT_TYPE_FUTEX_ENTER;
    hdr->flags = 0;

    bpf_ringbuf_submit(buf, 0);
    return 0;
}

// 8. syscalls:sys_exit_futex
SEC("tp/syscalls/sys_exit_futex")
int handle_sys_exit_futex(void *ctx) {
    u64 ptid = bpf_get_current_pid_tgid();
    u32 tid = (u32)ptid;
    if (!is_tracked(tid)) return 0;

    u64 s = next_seq();
    void *buf = bpf_ringbuf_reserve(&events, sizeof(struct kt_hdr) + sizeof(struct kt_futex_exit), 0);
    if (!buf) return 0;

    struct kt_hdr *hdr = (struct kt_hdr *)buf;
    hdr->ts_ns = bpf_ktime_get_ns();
    hdr->seq = s;
    hdr->tid = tid;
    hdr->tgid = (u32)(ptid >> 32);
    hdr->cpu = (u16)bpf_get_smp_processor_id();
    hdr->type = KT_TYPE_FUTEX_EXIT;
    hdr->flags = 0;

    bpf_ringbuf_submit(buf, 0);
    return 0;
}

// 9. block:block_rq_issue (Phase 2 Block I/O, PRD §32.1)
SEC("tp/block/block_rq_issue")
int handle_block_rq_issue(void *ctx) {
    u64 ptid = bpf_get_current_pid_tgid();
    u32 tid = (u32)ptid;
    if (!is_tracked(tid)) return 0;

    u64 s = next_seq();
    void *buf = bpf_ringbuf_reserve(&events, sizeof(struct kt_hdr) + sizeof(struct kt_block_issue), 0);
    if (!buf) return 0;

    struct kt_hdr *hdr = (struct kt_hdr *)buf;
    hdr->ts_ns = bpf_ktime_get_ns();
    hdr->seq = s;
    hdr->tid = tid;
    hdr->tgid = (u32)(ptid >> 32);
    hdr->cpu = (u16)bpf_get_smp_processor_id();
    hdr->type = KT_TYPE_BLOCK_ISSUE;
    hdr->flags = 0;

    bpf_ringbuf_submit(buf, 0);
    return 0;
}

// 10. block:block_rq_complete (Phase 2 Block I/O, PRD §32.1)
SEC("tp/block/block_rq_complete")
int handle_block_rq_complete(void *ctx) {
    u64 s = next_seq();
    void *buf = bpf_ringbuf_reserve(&events, sizeof(struct kt_hdr) + sizeof(struct kt_block_complete), 0);
    if (!buf) return 0;

    u64 ptid = bpf_get_current_pid_tgid();
    struct kt_hdr *hdr = (struct kt_hdr *)buf;
    hdr->ts_ns = bpf_ktime_get_ns();
    hdr->seq = s;
    hdr->tid = (u32)ptid;
    hdr->tgid = (u32)(ptid >> 32);
    hdr->cpu = (u16)bpf_get_smp_processor_id();
    hdr->type = KT_TYPE_BLOCK_COMPLETE;
    hdr->flags = 0;

    bpf_ringbuf_submit(buf, 0);
    return 0;
}

char LICENSE[] SEC("license") = "GPL";
