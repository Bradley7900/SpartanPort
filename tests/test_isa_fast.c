/* isa_rewrite fast stubs (fp32-gcn1): register-only BEXTR / BLSR sites. Each case builds a snippet
 * that loads all 15 general registers (not rsp) and the arithmetic flags from a state block, runs
 * one BEXTR or BLSR (random width, registers, operands), and stores every register and the flags
 * back. It runs three times: the real instruction (when this CPU has BMI1), the site patched with
 * the fast stub, and the site patched with the full stub (isa_emulate). Registers must match
 * exactly and the flags the instruction defines must match. */
#define _GNU_SOURCE
#include <assert.h>
#include <cpuid.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <sys/mman.h>
#endif
#include "isa_rewrite.h"

enum { PAGE = 4096, CASES = 200000 };
enum { IN = 0, FLAGS_IN = 16 * 8, OUT = 17 * 8, FLAGS_OUT = 33 * 8 };   /* state block offsets */
typedef void (ABI *Harness)(uint64_t *state);

static unsigned char *region, *image, *stubs;
static unsigned char pristine[PAGE];
static size_t site_at;                                   /* offset of the 5-byte site in image */

static uint64_t rng = 0x9e3779b97f4a7c15ull;
static uint64_t next(void) {
    rng ^= rng << 13;
    rng ^= rng >> 7;
    rng ^= rng << 17;
    return rng;
}

static void put(unsigned char **p, const uint8_t *b, size_t n) {
    memcpy(*p, b, n);
    *p += n;
}
static void put8(unsigned char **p, uint8_t v) { *(*p)++ = v; }
static void put32(unsigned char **p, uint32_t v) { for (int i = 0; i < 4; i++) put8(p, (uint8_t)(v >> 8 * i)); }

/* mov reg, [rdi+disp32] (8B) or mov [rdi+disp32], reg (89). */
static void mov_rdi(unsigned char **p, uint8_t opcode, int reg, uint32_t disp) {
    put8(p, (uint8_t)(0x48 | (reg >= 8 ? 4 : 0)));
    put8(p, opcode);
    put8(p, (uint8_t)(0x87 | (reg & 7) << 3));
    put32(p, disp);
}

/* The VEX-encoded site: BEXTR dst, src1, src2 or BLSR dst, src1 (register forms). */
static void put_site(unsigned char **p, uint8_t kind, int w64, int dst, int src1, int src2) {
    put8(p, 0xc4);
    if (kind == ISA_BEXTR) {
        put8(p, (uint8_t)((dst >= 8 ? 0 : 0x80) | 0x40 | (src1 >= 8 ? 0 : 0x20) | 0x02));
        put8(p, (uint8_t)((w64 ? 0x80 : 0) | (~src2 & 15) << 3));
        put8(p, 0xf7);
        put8(p, (uint8_t)(0xc0 | (dst & 7) << 3 | (src1 & 7)));
    } else {
        put8(p, (uint8_t)(0x80 | 0x40 | (src1 >= 8 ? 0 : 0x20) | 0x02));
        put8(p, (uint8_t)((w64 ? 0x80 : 0) | (~dst & 15) << 3));
        put8(p, 0xf3);
        put8(p, (uint8_t)(0xc8 | (src1 & 7)));
    }
}

static void build_harness(uint8_t kind, int w64, int dst, int src1, int src2) {
    memset(pristine, 0xcc, sizeof pristine);
    unsigned char *p = pristine;
    put(&p, (const uint8_t[]){0x53, 0x55, 0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57, 0x57}, 11);  /* push rbx rbp r12-r15 rdi */
    put(&p, (const uint8_t[]){0xff, 0xb7}, 2);                                     /* push qword [rdi+FLAGS_IN] */
    put32(&p, FLAGS_IN);
    put8(&p, 0x9d);                                                                /* popfq */
    for (int r = 0; r < 16; r++)
        if (r != 4 && r != 7) mov_rdi(&p, 0x8b, r, IN + 8 * r);
    mov_rdi(&p, 0x8b, 7, IN + 8 * 7);                                              /* rdi last */
    site_at = (size_t)(p - pristine);
    put_site(&p, kind, w64, dst, src1, src2);
    put(&p, (const uint8_t[]){0x48, 0x87, 0x3c, 0x24}, 4);                         /* xchg rdi, [rsp] */
    for (int r = 0; r < 16; r++)
        if (r != 4 && r != 7) mov_rdi(&p, 0x89, r, OUT + 8 * r);
    put8(&p, 0x9c);                                                                /* pushfq */
    put(&p, (const uint8_t[]){0x8f, 0x87}, 2);                                     /* pop qword [rdi+FLAGS_OUT] */
    put32(&p, FLAGS_OUT);
    put8(&p, 0x58);                                                                /* pop rax (the guest's rdi) */
    mov_rdi(&p, 0x89, 0, OUT + 8 * 7);
    put(&p, (const uint8_t[]){0x41, 0x5f, 0x41, 0x5e, 0x41, 0x5d, 0x41, 0x5c, 0x5d, 0x5b, 0xc3}, 11);       /* pop r15-r12 rbp rbx ; ret */
}

static uint64_t operand(void) {
    switch (next() % 8) {
    case 0: return 0;
    case 1: return ~0ull;
    case 2: return 1ull << (next() % 64);
    case 3: return (1ull << (next() % 64)) - 1;
    case 4: return next() & 0xffffffffull;
    default: return next();
    }
}

static uint64_t control(void) {
    /* start and length past both widths too; random high bits must be ignored */
    const uint64_t start = next() % 80, length = next() % 80;
    return (next() % 4 == 0 ? next() & ~0xffffull : 0) | length << 8 | start;
}

static void run(uint64_t *state, const uint64_t *in) {
    memcpy(state, in, 17 * 8);
    memset(state + 17, 0x5a, 17 * 8);
    ((Harness)(void *)image)(state);
}

static int has_bmi1(void) {
    unsigned a, b, c, d;
    return __get_cpuid_count(7, 0, &a, &b, &c, &d) && (b & (1u << 3));
}

static void check(const char *what, const uint64_t *got, const uint64_t *want, uint8_t kind, int w64,
                  int dst, int src1, int src2, const uint64_t *in) {
    int ok = 1;
    for (int r = 0; r < 16; r++)
        if (r != 4 && got[17 + r] != want[17 + r]) ok = 0;
    if (((got[33] ^ want[33]) & isa_defined_flags(kind)) != 0) ok = 0;
    if (ok) return;
    fprintf(stderr, "%s mismatch: %s%d dst=%d src1=%d src2=%d src=%#llx ctl=%#llx\n", what,
            isa_kind_name(kind), w64 ? 64 : 32, dst, src1, src2, (unsigned long long)in[src1],
            (unsigned long long)in[src2]);
    for (int r = 0; r < 16; r++)
        if (r != 4 && got[17 + r] != want[17 + r])
            fprintf(stderr, "  r%d: got %#llx want %#llx\n", r, (unsigned long long)got[17 + r],
                    (unsigned long long)want[17 + r]);
    fprintf(stderr, "  flags: got %#llx want %#llx (defined %#llx)\n", (unsigned long long)got[33],
            (unsigned long long)want[33], (unsigned long long)isa_defined_flags(kind));
    exit(1);
}

/* Patches the harness' site with the fast (1) or full (0) stub; returns the table. */
static IsaTable *patch(uint8_t kind, int w64, int dst, int src1, int src2, int fast) {
    memcpy(image, pristine, PAGE);
    memset(stubs, 0, PAGE);
    static IsaSite site;
    static uint64_t hits;
    memset(&site, 0, sizeof site);
    site.offset = site_at;
    site.length = 5;
    site.kind = kind;
    site.width = w64 ? 64 : 32;
    site.mode = ISA_MODE_JMP;
    site.dst = (uint8_t)dst;
    site.src1 = (uint8_t)src1;
    site.src2 = kind == ISA_BEXTR ? (uint8_t)src2 : ISA_NO_REG;
    site.mem_slot = ISA_MEM_NONE;
    site.base = site.index = ISA_NO_REG;
    memcpy(site.original, pristine + site_at, 5);
    static IsaTable t;
    hits = 0;
    t.sites = &site;
    t.hits = &hits;
    t.count = 1;
    t.image_size = PAGE;
    isa_rewrite_set_fast(fast);
    IsaReport r = isa_rewrite_apply(&t, image, stubs, 0xffffffffu);
    assert(r.rewritten == 1 && r.jmp == 1 && r.fast == (uint32_t)fast);
    assert(image[site_at] == 0xe9);
    return &t;
}

int main(void) {
#ifdef _WIN32
    region = VirtualAlloc(NULL, 2 * PAGE, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE);
    assert(region != NULL);
#else
    region = mmap(NULL, 2 * PAGE, PROT_READ | PROT_WRITE | PROT_EXEC, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    assert(region != MAP_FAILED);
#endif
    image = region;
    stubs = region + PAGE;
    const int native = has_bmi1();
    if (!native) puts("isa-fast-test: no BMI1 here, comparing the fast stub with isa_emulate only");
    static const int regs[] = {0, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15};
    uint64_t in[17], want[34], fast[34], full[34];
    unsigned long long fast_sites = 0;
    for (int n = 0; n < CASES; n++) {
        const uint8_t kind = next() % 2 ? ISA_BEXTR : ISA_BLSR;
        const int w64 = (int)(next() % 2);
        const int dst = regs[next() % 15], src1 = regs[next() % 15], src2 = regs[next() % 15];
        for (int r = 0; r < 16; r++) in[r] = operand();
        in[4] = 0;
        if (kind == ISA_BEXTR) in[src2] = control();
        if (src2 == src1 && kind == ISA_BEXTR && next() % 2) in[src1] = control();
        in[16] = (next() & 0x8d5) | 0x2;                  /* CF PF AF ZF SF OF, reserved bit 1 */
        build_harness(kind, w64, dst, src1, src2);
        IsaSite probe = {.kind = kind, .width = (uint8_t)(w64 ? 64 : 32), .dst = (uint8_t)dst,
                         .src1 = (uint8_t)src1, .src2 = (uint8_t)(kind == ISA_BEXTR ? src2 : ISA_NO_REG),
                         .mem_slot = ISA_MEM_NONE};
        assert(isa_site_fast_eligible(&probe));
        IsaTable *t = patch(kind, w64, dst, src1, src2, 0);
        run(full, in);
        assert(t->hits[0] == 1);
        t = patch(kind, w64, dst, src1, src2, 1);
        run(fast, in);
        run(fast, in);
        assert(t->hits[0] == 2);
        fast_sites++;
        if (native) {
            memcpy(image, pristine, PAGE);
            run(want, in);
            check("full stub vs native", full, want, kind, w64, dst, src1, src2, in);
            check("fast stub vs native", fast, want, kind, w64, dst, src1, src2, in);
        }
        check("fast stub vs full stub", fast, full, kind, w64, dst, src1, src2, in);
    }
    /* Not eligible: memory operands, 16-bit, rsp, other kinds. */
    IsaSite s = {.kind = ISA_BEXTR, .width = 32, .dst = 0, .src1 = 1, .src2 = 2, .mem_slot = ISA_MEM_NONE};
    assert(isa_site_fast_eligible(&s));
    s.mem_slot = ISA_MEM_SRC, s.src1 = ISA_NO_REG;
    assert(!isa_site_fast_eligible(&s));
    s = (IsaSite){.kind = ISA_BLSR, .width = 16, .dst = 0, .src1 = 1, .mem_slot = ISA_MEM_NONE};
    assert(!isa_site_fast_eligible(&s));
    s.width = 64, s.src1 = 4;
    assert(!isa_site_fast_eligible(&s));
    s.src1 = 1, s.kind = ISA_ANDN;
    assert(!isa_site_fast_eligible(&s));
    printf("isa-fast-test ok: %llu cases%s\n", fast_sites, native ? ", checked against the CPU" : "");
    return 0;
}
