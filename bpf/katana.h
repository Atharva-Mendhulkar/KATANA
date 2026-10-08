#ifndef __KATANA_H__
#define __KATANA_H__

typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef unsigned long long u64;
typedef long long s64;
typedef int s32;

enum kt_type {
    KT_TYPE_SWITCH = 0,
    KT_TYPE_WAKING = 1,
    KT_TYPE_WAKEUP = 2,
    KT_TYPE_WAKEUP_NEW = 3,
    KT_TYPE_FORK = 4,
    KT_TYPE_EXIT = 5,
    KT_TYPE_TRACK_ADD = 6,
    KT_TYPE_FUTEX_ENTER = 7,
    KT_TYPE_FUTEX_EXIT = 8,
    KT_TYPE_BLOCK_ISSUE = 9,
    KT_TYPE_BLOCK_COMPLETE = 10,
    KT_TYPE_UNSUPPORTED_SYSCALL = 11,
};

#define KT_F_WAKER_IRQ       (1 << 0)
#define KT_F_WAKER_KTHREAD   (1 << 1)
#define KT_F_EXPAND_DENIED   (1 << 2)
#define KT_F_TRUNC           (1 << 3)

struct kt_hdr {
    u64 ts_ns;
    u64 seq;
    u32 tid;
    u32 tgid;
    u16 cpu;
    u8  type;
    u8  flags;
} __attribute__((packed));

struct kt_switch {
    u32 prev_tid;
    u32 next_tid;
    u32 prev_state;
    u16 sflags;
    u16 pad;
    u32 next_tgid;
    u32 prev_tgid;
} __attribute__((packed));

struct kt_wake {
    u32 wakee_tid;
    u32 wakee_tgid;
    u16 target_cpu;
    u16 sflags;
    u32 depth_added;
} __attribute__((packed));

struct kt_futex_enter {
    u64 uaddr;
    u64 uaddr2;
    u32 op;
    u32 val;
    u32 val3;
    u32 pi_word;
    u8  pi_word_valid;
    u8  has_timeout;
    u16 pad;
} __attribute__((packed));

struct kt_futex_exit {
    s64 ret;
} __attribute__((packed));

struct kt_block_issue {
    u32 dev_id;
    u32 nr_sector;
    u64 req_id;
    u64 sector;
    u32 submitter_tid;
    u32 submitter_tgid;
    char rwbs[8];
} __attribute__((packed));

struct kt_block_complete {
    u32 dev_id;
    u32 nr_bytes;
    u64 req_id;
    s32 error;
    u32 pad;
} __attribute__((packed));

struct kt_tracked {
    u64 start_boottime;
    u32 tgid;
    u8 depth;
    u8 flags;
};

#endif /* __KATANA_H__ */
