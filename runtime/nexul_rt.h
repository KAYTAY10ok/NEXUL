/*
 * NEXUL runtime 0.1
 *
 * This file is embedded in the compiler and copied at the top of every generated C
 * translation unit. Everything is `static`: the C compiler keeps only what a program
 * uses (spec §5.3: modular runtime). It requires a C11 compiler with the GCC/Clang
 * overflow builtins (`zig cc`, clang, gcc).
 */
/* POSIX interfaces (clock_gettime) are only declared on request in strict C11 mode. */
#if !defined(_WIN32) && !defined(_POSIX_C_SOURCE)
#define _POSIX_C_SOURCE 200809L
#endif

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <time.h>

#if defined(_WIN32)
#include <wchar.h>
#endif

#if defined(__GNUC__) || defined(__clang__)
#define NX_NORETURN __attribute__((noreturn))
#define NX_UNREACHABLE() __builtin_unreachable()
#define NX_UNUSED __attribute__((unused))
#define NX_LIKELY(x) __builtin_expect(!!(x), 1)
#else
#define NX_NORETURN _Noreturn
#define NX_UNREACHABLE() abort()
#define NX_UNUSED
#define NX_LIKELY(x) (x)
#endif

/* Symbol name of a C function, for `extern "C"` declarations (avoids clashes with
   the declarations of the C headers included above). */
#define NX_STR_(x) #x
#define NX_STR(x) NX_STR_(x)
#ifdef __USER_LABEL_PREFIX__
#define NX_SYM(name) __asm__(NX_STR(__USER_LABEL_PREFIX__) name)
#else
#define NX_SYM(name) __asm__(name)
#endif

typedef uint8_t nx_unit;

typedef struct nx_loc {
    const char* file;
    uint32_t line;
    uint32_t col;
} nx_loc;

/* Must match `struct String` in lib/std/string.nx. `cap == 0` means the bytes are not
   owned (string literal or empty string): they are never freed nor written. */
typedef struct nx_String {
    uint8_t* f_data;
    size_t f_len;
    size_t f_cap;
} nx_String;

static int nx_argc;
static char** nx_argv;

/* ------------------------------------------------------------------------- */
/* Panics                                                                     */

static void nx_write_err(const char* s, size_t n) {
    fwrite(s, 1, n, stderr);
}

static void nx_u64_to_dec(uint64_t v, char* buf, size_t* len) {
    char tmp[24];
    size_t n = 0;
    do {
        tmp[n++] = (char)('0' + (v % 10));
        v /= 10;
    } while (v);
    for (size_t i = 0; i < n; i++) buf[i] = tmp[n - 1 - i];
    *len = n;
}

NX_NORETURN static void nx_panic_bytes(const char* msg, size_t len, const nx_loc* loc) {
    fflush(stdout);
    nx_write_err("panic: ", 7);
    nx_write_err(msg, len);
    nx_write_err("\n", 1);
    if (loc && loc->file) {
        char b1[24], b2[24];
        size_t n1, n2;
        nx_u64_to_dec(loc->line, b1, &n1);
        nx_u64_to_dec(loc->col, b2, &n2);
        nx_write_err("  --> ", 6);
        nx_write_err(loc->file, strlen(loc->file));
        nx_write_err(":", 1);
        nx_write_err(b1, n1);
        nx_write_err(":", 1);
        nx_write_err(b2, n2);
        nx_write_err("\n", 1);
    }
    fflush(stderr);
    exit(101);
}

NX_NORETURN static NX_UNUSED void nx_panic(const char* msg, const nx_loc* loc) {
    nx_panic_bytes(msg, strlen(msg), loc);
}

NX_NORETURN static NX_UNUSED void nx_panic_str(const nx_String* s, const nx_loc* loc) {
    nx_panic_bytes((const char*)s->f_data, s->f_len, loc);
}

/* "prefix<a>middle<b>" with two unsigned numbers. */
NX_NORETURN static void nx_panic_2u(const char* prefix, uint64_t a, const char* middle, uint64_t b, const nx_loc* loc) {
    char buf[256];
    size_t n = 0, k;
    size_t pl = strlen(prefix), ml = strlen(middle);
    memcpy(buf, prefix, pl);
    n += pl;
    nx_u64_to_dec(a, buf + n, &k);
    n += k;
    memcpy(buf + n, middle, ml);
    n += ml;
    nx_u64_to_dec(b, buf + n, &k);
    n += k;
    nx_panic_bytes(buf, n, loc);
}

NX_NORETURN static void nx_panic_index_neg(int64_t i, uint64_t len, const nx_loc* loc) {
    char buf[256];
    size_t n = 0, k;
    const char* p = "index out of bounds: the index is -";
    memcpy(buf, p, strlen(p));
    n += strlen(p);
    nx_u64_to_dec((uint64_t)(-(i + 1)) + 1, buf + n, &k);
    n += k;
    const char* m = " but the length is ";
    memcpy(buf + n, m, strlen(m));
    n += strlen(m);
    nx_u64_to_dec(len, buf + n, &k);
    n += k;
    nx_panic_bytes(buf, n, loc);
}

/* ------------------------------------------------------------------------- */
/* Memory                                                                     */

/* Number of live heap allocations, checked at exit when the environment variable
   NEXUL_LEAK_CHECK is set (used by the test suite to verify destruction). */
static long long nx_live_allocs;

static NX_UNUSED void* nx_alloc(size_t size) {
    void* p = malloc(size ? size : 1);
    if (!p) nx_panic("out of memory", NULL);
    nx_live_allocs++;
    return p;
}

static NX_UNUSED void* nx_realloc(void* old, size_t size) {
    void* p = realloc(old, size ? size : 1);
    if (!p) nx_panic("out of memory", NULL);
    if (!old) nx_live_allocs++;
    return p;
}

static NX_UNUSED void nx_free(void* p) {
    if (p) nx_live_allocs--;
    free(p);
}

/* ------------------------------------------------------------------------- */
/* Checked integer arithmetic (spec §4.2)                                     */

#define NX_INT_OPS(T, N, UT, MIN, MAX, SIGNED)                                                       \
    static inline NX_UNUSED T nx_add_##N(T a, T b, const nx_loc* l) {                              \
        T r;                                                                                        \
        if (__builtin_add_overflow(a, b, &r)) nx_panic("attempt to add with overflow", l);          \
        return r;                                                                                   \
    }                                                                                               \
    static inline NX_UNUSED T nx_sub_##N(T a, T b, const nx_loc* l) {                              \
        T r;                                                                                        \
        if (__builtin_sub_overflow(a, b, &r)) nx_panic("attempt to subtract with overflow", l);     \
        return r;                                                                                   \
    }                                                                                               \
    static inline NX_UNUSED T nx_mul_##N(T a, T b, const nx_loc* l) {                              \
        T r;                                                                                        \
        if (__builtin_mul_overflow(a, b, &r)) nx_panic("attempt to multiply with overflow", l);     \
        return r;                                                                                   \
    }                                                                                               \
    static inline NX_UNUSED T nx_div_##N(T a, T b, const nx_loc* l) {                              \
        if (b == 0) nx_panic("attempt to divide by zero", l);                                       \
        if (SIGNED && a == (T)(MIN) && b == (T)-1) nx_panic("attempt to divide with overflow", l);  \
        return (T)(a / b);                                                                          \
    }                                                                                               \
    static inline NX_UNUSED T nx_rem_##N(T a, T b, const nx_loc* l) {                              \
        if (b == 0) nx_panic("attempt to calculate the remainder with a divisor of zero", l);      \
        if (SIGNED && a == (T)(MIN) && b == (T)-1)                                                  \
            nx_panic("attempt to calculate the remainder with overflow", l);                        \
        return (T)(a % b);                                                                          \
    }                                                                                               \
    static inline NX_UNUSED T nx_neg_##N(T a, const nx_loc* l) {                                   \
        if (!SIGNED || a == (T)(MIN)) nx_panic("attempt to negate with overflow", l);               \
        return (T)(-a);                                                                             \
    }                                                                                               \
    static inline NX_UNUSED T nx_shl_##N(T a, int64_t n, const nx_loc* l) {                        \
        if (n < 0 || n >= (int64_t)(sizeof(T) * 8)) nx_panic("attempt to shift left with overflow", l); \
        return (T)((UT)a << n);                                                                     \
    }                                                                                               \
    static inline NX_UNUSED T nx_shr_##N(T a, int64_t n, const nx_loc* l) {                        \
        if (n < 0 || n >= (int64_t)(sizeof(T) * 8)) nx_panic("attempt to shift right with overflow", l); \
        return (T)(a >> n);                                                                         \
    }                                                                                               \
    static inline NX_UNUSED T nx_abs_##N(T a, const nx_loc* l) {                                   \
        if (SIGNED && a == (T)(MIN)) nx_panic("attempt to take the absolute value with overflow", l); \
        return (SIGNED && a < 0) ? (T)(-a) : a;                                                     \
    }                                                                                               \
    static inline NX_UNUSED T nx_pow_##N(T base, uint32_t exp, const nx_loc* l) {                  \
        T r = 1;                                                                                    \
        while (exp) {                                                                               \
            if (exp & 1) r = nx_mul_##N(r, base, l);                                                \
            exp >>= 1;                                                                              \
            if (exp) base = nx_mul_##N(base, base, l);                                              \
        }                                                                                           \
        return r;                                                                                   \
    }

NX_INT_OPS(int8_t, i8, uint8_t, INT8_MIN, INT8_MAX, 1)
NX_INT_OPS(int16_t, i16, uint16_t, INT16_MIN, INT16_MAX, 1)
NX_INT_OPS(int32_t, i32, uint32_t, INT32_MIN, INT32_MAX, 1)
NX_INT_OPS(int64_t, i64, uint64_t, INT64_MIN, INT64_MAX, 1)
NX_INT_OPS(intptr_t, isize, uintptr_t, INTPTR_MIN, INTPTR_MAX, 1)
NX_INT_OPS(uint8_t, u8, uint8_t, 0, UINT8_MAX, 0)
NX_INT_OPS(uint16_t, u16, uint16_t, 0, UINT16_MAX, 0)
NX_INT_OPS(uint32_t, u32, uint32_t, 0, UINT32_MAX, 0)
NX_INT_OPS(uint64_t, u64, uint64_t, 0, UINT64_MAX, 0)
NX_INT_OPS(uintptr_t, usize, uintptr_t, 0, UINTPTR_MAX, 0)

/* ------------------------------------------------------------------------- */
/* Checked conversions (spec §3.6)                                            */

static inline NX_UNUSED int64_t nx_cast_ss(int64_t v, int64_t min, int64_t max, const nx_loc* l) {
    if (v < min || v > max) nx_panic("value out of range in `as` conversion", l);
    return v;
}
static inline NX_UNUSED uint64_t nx_cast_su(int64_t v, uint64_t max, const nx_loc* l) {
    if (v < 0 || (uint64_t)v > max) nx_panic("value out of range in `as` conversion", l);
    return (uint64_t)v;
}
static inline NX_UNUSED int64_t nx_cast_us(uint64_t v, int64_t max, const nx_loc* l) {
    if (v > (uint64_t)max) nx_panic("value out of range in `as` conversion", l);
    return (int64_t)v;
}
static inline NX_UNUSED uint64_t nx_cast_uu(uint64_t v, uint64_t max, const nx_loc* l) {
    if (v > max) nx_panic("value out of range in `as` conversion", l);
    return v;
}
/* float -> signed integer of `bits` bits, truncating toward zero */
static inline NX_UNUSED int64_t nx_cast_fs(double v, int bits, const nx_loc* l) {
    double hi = ldexp(1.0, bits - 1);
    double lo = -hi;
    bool ok = bits == 64 ? (v >= lo && v < hi) : (v > lo - 1.0 && v < hi);
    if (!ok) nx_panic(isnan(v) ? "cannot convert NaN to an integer" : "value out of range in `as` conversion", l);
    return (int64_t)v;
}
static inline NX_UNUSED uint64_t nx_cast_fu(double v, int bits, const nx_loc* l) {
    double hi = ldexp(1.0, bits);
    if (!(v > -1.0 && v < hi)) nx_panic(isnan(v) ? "cannot convert NaN to an integer" : "value out of range in `as` conversion", l);
    return (uint64_t)v;
}
static inline NX_UNUSED uint32_t nx_cast_char_s(int64_t v, const nx_loc* l) {
    if (v < 0 || v > 0x10FFFF || (v >= 0xD800 && v <= 0xDFFF)) nx_panic("value is not a valid `char`", l);
    return (uint32_t)v;
}
static inline NX_UNUSED uint32_t nx_cast_char_u(uint64_t v, const nx_loc* l) {
    if (v > 0x10FFFF || (v >= 0xD800 && v <= 0xDFFF)) nx_panic("value is not a valid `char`", l);
    return (uint32_t)v;
}

/* ------------------------------------------------------------------------- */
/* Bounds checks                                                              */

static inline NX_UNUSED size_t nx_index_s(int64_t i, size_t len, const nx_loc* l) {
    if (i < 0) nx_panic_index_neg(i, len, l);
    if ((uint64_t)i >= (uint64_t)len) nx_panic_2u("index out of bounds: the index is ", (uint64_t)i, " but the length is ", len, l);
    return (size_t)i;
}
static inline NX_UNUSED size_t nx_index_u(uint64_t i, size_t len, const nx_loc* l) {
    if (i >= (uint64_t)len) nx_panic_2u("index out of bounds: the index is ", i, " but the length is ", len, l);
    return (size_t)i;
}

/* ------------------------------------------------------------------------- */
/* Strings                                                                    */

#define nx_str_lit(data, len) ((nx_String){(uint8_t*)(data), (size_t)(len), 0})

static NX_UNUSED nx_String nx_str_empty(void) {
    return nx_str_lit("", 0);
}

/* Makes room for `extra` more bytes, copying non-owned contents to the heap. */
static void nx_str_reserve(nx_String* s, size_t extra) {
    size_t need = s->f_len + extra;
    if (need < s->f_len) nx_panic("capacity overflow", NULL);
    if (s->f_cap >= need && s->f_cap != 0) return;
    size_t cap = s->f_cap ? s->f_cap * 2 : 16;
    if (cap < need) cap = need;
    if (s->f_cap == 0) {
        uint8_t* p = (uint8_t*)nx_alloc(cap);
        if (s->f_len) memcpy(p, s->f_data, s->f_len);
        s->f_data = p;
    } else {
        s->f_data = (uint8_t*)nx_realloc(s->f_data, cap);
    }
    s->f_cap = cap;
}

static void nx_str_push_bytes(nx_String* s, const uint8_t* b, size_t n) {
    if (n == 0) return;
    nx_str_reserve(s, n);
    memcpy(s->f_data + s->f_len, b, n);
    s->f_len += n;
}

static NX_UNUSED void nx_str_push_str(nx_String* s, const nx_String* t) {
    if (s == t) {
        nx_String copy = *t;
        size_t n = copy.f_len;
        nx_str_reserve(s, n);
        memmove(s->f_data + s->f_len, s->f_data, n);
        s->f_len += n;
        return;
    }
    nx_str_push_bytes(s, t->f_data, t->f_len);
}

static void nx_str_push_char(nx_String* s, uint32_t c) {
    uint8_t b[4];
    size_t n;
    if (c < 0x80) {
        b[0] = (uint8_t)c;
        n = 1;
    } else if (c < 0x800) {
        b[0] = (uint8_t)(0xC0 | (c >> 6));
        b[1] = (uint8_t)(0x80 | (c & 0x3F));
        n = 2;
    } else if (c < 0x10000) {
        b[0] = (uint8_t)(0xE0 | (c >> 12));
        b[1] = (uint8_t)(0x80 | ((c >> 6) & 0x3F));
        b[2] = (uint8_t)(0x80 | (c & 0x3F));
        n = 3;
    } else {
        b[0] = (uint8_t)(0xF0 | (c >> 18));
        b[1] = (uint8_t)(0x80 | ((c >> 12) & 0x3F));
        b[2] = (uint8_t)(0x80 | ((c >> 6) & 0x3F));
        b[3] = (uint8_t)(0x80 | (c & 0x3F));
        n = 4;
    }
    nx_str_push_bytes(s, b, n);
}

static NX_UNUSED nx_String nx_str_concat(const nx_String* a, const nx_String* b) {
    nx_String r = nx_str_empty();
    nx_str_reserve(&r, a->f_len + b->f_len);
    nx_str_push_bytes(&r, a->f_data, a->f_len);
    nx_str_push_bytes(&r, b->f_data, b->f_len);
    return r;
}

static NX_UNUSED bool nx_str_eq(const nx_String* a, const nx_String* b) {
    return a->f_len == b->f_len && (a->f_len == 0 || memcmp(a->f_data, b->f_data, a->f_len) == 0);
}

static NX_UNUSED int32_t nx_str_cmp(const nx_String* a, const nx_String* b) {
    size_t n = a->f_len < b->f_len ? a->f_len : b->f_len;
    int c = n ? memcmp(a->f_data, b->f_data, n) : 0;
    if (c != 0) return c < 0 ? -1 : 1;
    if (a->f_len == b->f_len) return 0;
    return a->f_len < b->f_len ? -1 : 1;
}

/* Decodes the (valid) UTF-8 character at *pos and advances *pos. */
static NX_UNUSED uint32_t nx_str_decode(const nx_String* s, size_t* pos) {
    const uint8_t* p = s->f_data + *pos;
    uint32_t c = p[0];
    if (c < 0x80) {
        *pos += 1;
        return c;
    }
    if (c < 0xE0) {
        *pos += 2;
        return ((c & 0x1F) << 6) | (p[1] & 0x3F);
    }
    if (c < 0xF0) {
        *pos += 3;
        return ((c & 0x0F) << 12) | ((uint32_t)(p[1] & 0x3F) << 6) | (p[2] & 0x3F);
    }
    *pos += 4;
    return ((c & 0x07) << 18) | ((uint32_t)(p[1] & 0x3F) << 12) | ((uint32_t)(p[2] & 0x3F) << 6) | (p[3] & 0x3F);
}

/* Length of the valid UTF-8 prefix; invalid sequences are replaced by U+FFFD. */
static NX_UNUSED nx_String nx_str_from_bytes_lossy(const uint8_t* b, size_t n) {
    nx_String r = nx_str_empty();
    size_t i = 0;
    nx_str_reserve(&r, n);
    while (i < n) {
        uint8_t c = b[i];
        size_t len = 0;
        uint32_t min = 0;
        if (c < 0x80) len = 1;
        else if ((c & 0xE0) == 0xC0) { len = 2; min = 0x80; }
        else if ((c & 0xF0) == 0xE0) { len = 3; min = 0x800; }
        else if ((c & 0xF8) == 0xF0) { len = 4; min = 0x10000; }
        bool ok = len > 0 && i + len <= n;
        uint32_t cp = 0;
        if (ok && len > 1) {
            cp = c & (0xFF >> (len + 1));
            for (size_t k = 1; k < len; k++) {
                if ((b[i + k] & 0xC0) != 0x80) { ok = false; break; }
                cp = (cp << 6) | (b[i + k] & 0x3F);
            }
            if (ok && (cp < min || cp > 0x10FFFF || (cp >= 0xD800 && cp <= 0xDFFF))) ok = false;
        }
        if (ok) {
            nx_str_push_bytes(&r, b + i, len);
            i += len;
        } else {
            nx_str_push_char(&r, 0xFFFD);
            i += 1;
        }
    }
    return r;
}

static NX_UNUSED bool nx_utf8_valid(const uint8_t* b, size_t n) {
    nx_String t = nx_str_from_bytes_lossy(b, n);
    bool same = t.f_len == n && (n == 0 || memcmp(t.f_data, b, n) == 0);
    if (t.f_cap) nx_free(t.f_data);
    return same;
}

/* ------------------------------------------------------------------------- */
/* Formatting (spec §1.5, §4.11)                                              */

typedef struct nx_spec {
    uint32_t fill;
    uint8_t align; /* 0 default, 1 left, 2 center, 3 right */
    bool plus;
    bool zero;
    int32_t width;     /* -1: none */
    int32_t precision; /* -1: none */
    uint8_t kind;      /* 0 default, 1 x, 2 X, 3 o, 4 b, 5 e */
} nx_spec;

static size_t nx_utf8_count(const uint8_t* s, size_t n) {
    size_t c = 0;
    for (size_t i = 0; i < n; i++)
        if ((s[i] & 0xC0) != 0x80) c++;
    return c;
}

/* Appends `s` with padding; `sign_len` leading bytes stay before zero padding. */
static void nx_fmt_pad(nx_String* d, const uint8_t* s, size_t n, const nx_spec* sp, bool numeric, size_t sign_len) {
    if (!sp || sp->width < 0) {
        nx_str_push_bytes(d, s, n);
        return;
    }
    size_t chars = nx_utf8_count(s, n);
    size_t w = (size_t)sp->width;
    if (chars >= w) {
        nx_str_push_bytes(d, s, n);
        return;
    }
    size_t pad = w - chars;
    if (numeric && sp->zero && sp->align == 0) {
        nx_str_push_bytes(d, s, sign_len);
        for (size_t i = 0; i < pad; i++) nx_str_push_char(d, '0');
        nx_str_push_bytes(d, s + sign_len, n - sign_len);
        return;
    }
    uint8_t align = sp->align ? sp->align : (numeric ? 3 : 1);
    size_t left = align == 1 ? 0 : align == 3 ? pad : pad / 2;
    size_t right = pad - left;
    uint32_t fill = sp->fill ? sp->fill : ' ';
    for (size_t i = 0; i < left; i++) nx_str_push_char(d, fill);
    nx_str_push_bytes(d, s, n);
    for (size_t i = 0; i < right; i++) nx_str_push_char(d, fill);
}

static void nx_fmt_uint(nx_String* d, uint64_t v, bool neg, const nx_spec* sp) {
    char buf[80];
    size_t n = 0;
    if (neg) buf[n++] = '-';
    else if (sp && sp->plus) buf[n++] = '+';
    size_t sign = n;
    unsigned base = 10;
    const char* digits = "0123456789abcdef";
    if (sp) {
        if (sp->kind == 1) base = 16;
        else if (sp->kind == 2) { base = 16; digits = "0123456789ABCDEF"; }
        else if (sp->kind == 3) base = 8;
        else if (sp->kind == 4) base = 2;
    }
    char tmp[70];
    size_t k = 0;
    do {
        tmp[k++] = digits[v % base];
        v /= base;
    } while (v);
    while (k) buf[n++] = tmp[--k];
    nx_fmt_pad(d, (const uint8_t*)buf, n, sp, true, sign);
}

static NX_UNUSED void nx_fmt_i64(nx_String* d, int64_t v, const nx_spec* sp) {
    if (v < 0) nx_fmt_uint(d, (uint64_t)(-(v + 1)) + 1, true, sp);
    else nx_fmt_uint(d, (uint64_t)v, false, sp);
}

static NX_UNUSED void nx_fmt_u64(nx_String* d, uint64_t v, const nx_spec* sp) {
    nx_fmt_uint(d, v, false, sp);
}

/* Shortest representation that reads back to the same value. */
static size_t nx_float_shortest(double v, bool is_f32, bool sci, char* out) {
    char buf[64];
    int p;
    for (p = 1; p <= 17; p++) {
        snprintf(buf, sizeof buf, "%.*e", p - 1, v);
        double back = is_f32 ? (double)strtof(buf, NULL) : strtod(buf, NULL);
        if (back == v) break;
    }
    /* buf = [-]d[.ddd]e[+-]XX */
    char digits[32];
    size_t nd = 0;
    const char* s = buf;
    bool neg = false;
    if (*s == '-') { neg = true; s++; }
    while (*s && *s != 'e') {
        if (*s != '.') digits[nd++] = *s;
        s++;
    }
    int exp = atoi(s + 1);
    while (nd > 1 && digits[nd - 1] == '0') nd--;
    size_t n = 0;
    if (neg) out[n++] = '-';
    if (sci || exp < -5 || exp >= 17) {
        out[n++] = digits[0];
        if (nd > 1) {
            out[n++] = '.';
            for (size_t i = 1; i < nd; i++) out[n++] = digits[i];
        }
        n += (size_t)snprintf(out + n, 16, "e%d", exp);
        return n;
    }
    if (exp >= 0) {
        for (int i = 0; i <= exp; i++) out[n++] = (size_t)i < nd ? digits[i] : '0';
        out[n++] = '.';
        if ((size_t)exp + 1 < nd) {
            for (size_t i = (size_t)exp + 1; i < nd; i++) out[n++] = digits[i];
        } else {
            out[n++] = '0';
        }
    } else {
        out[n++] = '0';
        out[n++] = '.';
        for (int i = 0; i < -exp - 1; i++) out[n++] = '0';
        for (size_t i = 0; i < nd; i++) out[n++] = digits[i];
    }
    return n;
}

static NX_UNUSED void nx_fmt_f64(nx_String* d, double v, bool is_f32, const nx_spec* sp) {
    char buf[400];
    size_t n = 0;
    if (isnan(v)) {
        memcpy(buf, "NaN", 3);
        n = 3;
        nx_fmt_pad(d, (const uint8_t*)buf, n, sp, true, 0);
        return;
    }
    bool neg = signbit(v) != 0;
    if (isinf(v)) {
        n = (size_t)snprintf(buf, sizeof buf, "%s%sinf", neg ? "-" : "", (!neg && sp && sp->plus) ? "+" : "");
        nx_fmt_pad(d, (const uint8_t*)buf, n, sp, true, 0);
        return;
    }
    bool sci = sp && sp->kind == 5;
    if (sp && sp->precision >= 0) {
        n = (size_t)snprintf(buf, sizeof buf, sci ? "%.*e" : "%.*f", sp->precision, v);
        if (sci) {
            /* normalize the exponent: e+05 -> e5 */
            char* e = strchr(buf, 'e');
            if (e) {
                int exp = atoi(e + 1);
                n = (size_t)(e - buf) + (size_t)snprintf(e, 16, "e%d", exp);
            }
        }
    } else {
        n = nx_float_shortest(v, is_f32, sci, buf);
    }
    size_t sign = 0;
    if (buf[0] == '-') sign = 1;
    else if (sp && sp->plus) {
        memmove(buf + 1, buf, n);
        buf[0] = '+';
        n++;
        sign = 1;
    }
    nx_fmt_pad(d, (const uint8_t*)buf, n, sp, true, sign);
}

static NX_UNUSED void nx_fmt_bool(nx_String* d, bool v, const nx_spec* sp) {
    if (v) nx_fmt_pad(d, (const uint8_t*)"true", 4, sp, false, 0);
    else nx_fmt_pad(d, (const uint8_t*)"false", 5, sp, false, 0);
}

static NX_UNUSED void nx_fmt_char(nx_String* d, uint32_t c, const nx_spec* sp) {
    nx_String t = nx_str_empty();
    nx_str_push_char(&t, c);
    nx_fmt_pad(d, t.f_data, t.f_len, sp, false, 0);
    if (t.f_cap) nx_free(t.f_data);
}

static NX_UNUSED void nx_fmt_str(nx_String* d, const nx_String* s, const nx_spec* sp) {
    if (sp && sp->precision >= 0) {
        /* precision = maximum number of characters */
        size_t chars = 0, i = 0;
        while (i < s->f_len && chars < (size_t)sp->precision) {
            i++;
            while (i < s->f_len && (s->f_data[i] & 0xC0) == 0x80) i++;
            chars++;
        }
        nx_fmt_pad(d, s->f_data, i, sp, false, 0);
        return;
    }
    if (d == s) {
        nx_String copy = nx_str_concat(s, &(nx_String){(uint8_t*)"", 0, 0});
        nx_fmt_pad(d, copy.f_data, copy.f_len, sp, false, 0);
        if (copy.f_cap) nx_free(copy.f_data);
        return;
    }
    nx_fmt_pad(d, s->f_data, s->f_len, sp, false, 0);
}

static NX_UNUSED nx_String nx_to_string_i64(int64_t v) {
    nx_String s = nx_str_empty();
    nx_fmt_i64(&s, v, NULL);
    return s;
}
static NX_UNUSED nx_String nx_to_string_u64(uint64_t v) {
    nx_String s = nx_str_empty();
    nx_fmt_u64(&s, v, NULL);
    return s;
}
static NX_UNUSED nx_String nx_to_string_f64(double v, bool is_f32) {
    nx_String s = nx_str_empty();
    nx_fmt_f64(&s, v, is_f32, NULL);
    return s;
}
static NX_UNUSED nx_String nx_to_string_bool(bool v) {
    return v ? nx_str_lit("true", 4) : nx_str_lit("false", 5);
}
static NX_UNUSED nx_String nx_to_string_char(uint32_t c) {
    nx_String s = nx_str_empty();
    nx_str_push_char(&s, c);
    return s;
}

/* ------------------------------------------------------------------------- */
/* Characters                                                                 */

static inline NX_UNUSED bool nx_char_is_ascii_digit(uint32_t c) { return c >= '0' && c <= '9'; }
static inline NX_UNUSED bool nx_char_is_ascii_uppercase(uint32_t c) { return c >= 'A' && c <= 'Z'; }
static inline NX_UNUSED bool nx_char_is_ascii_lowercase(uint32_t c) { return c >= 'a' && c <= 'z'; }
static inline NX_UNUSED bool nx_char_is_ascii_alphabetic(uint32_t c) { return nx_char_is_ascii_uppercase(c) || nx_char_is_ascii_lowercase(c); }
static inline NX_UNUSED bool nx_char_is_ascii_alphanumeric(uint32_t c) { return nx_char_is_ascii_alphabetic(c) || nx_char_is_ascii_digit(c); }
static inline NX_UNUSED bool nx_char_is_ascii(uint32_t c) { return c < 0x80; }
static inline NX_UNUSED bool nx_char_is_whitespace(uint32_t c) {
    return c == ' ' || (c >= 0x09 && c <= 0x0D) || c == 0x85 || c == 0xA0 || c == 0x1680 || (c >= 0x2000 && c <= 0x200A) ||
           c == 0x2028 || c == 0x2029 || c == 0x202F || c == 0x205F || c == 0x3000;
}
static inline NX_UNUSED uint32_t nx_char_to_upper(uint32_t c) { return nx_char_is_ascii_lowercase(c) ? c - 32 : c; }
static inline NX_UNUSED uint32_t nx_char_to_lower(uint32_t c) { return nx_char_is_ascii_uppercase(c) ? c + 32 : c; }
static inline NX_UNUSED size_t nx_char_len_utf8(uint32_t c) { return c < 0x80 ? 1 : c < 0x800 ? 2 : c < 0x10000 ? 3 : 4; }

/* ------------------------------------------------------------------------- */
/* Input / output and system                                                 */

#if defined(_WIN32)
__declspec(dllimport) int __stdcall SetConsoleOutputCP(unsigned int);
__declspec(dllimport) int __stdcall MultiByteToWideChar(unsigned int, unsigned long, const char*, int, wchar_t*, int);
__declspec(dllimport) int __stdcall WideCharToMultiByte(unsigned int, unsigned long, const wchar_t*, int, char*, int, const char*, int*);
__declspec(dllimport) wchar_t* __stdcall GetCommandLineW(void);
__declspec(dllimport) wchar_t** __stdcall CommandLineToArgvW(const wchar_t*, int*);
__declspec(dllimport) int __stdcall QueryPerformanceCounter(long long*);
__declspec(dllimport) int __stdcall QueryPerformanceFrequency(long long*);

static wchar_t* nx_utf8_to_wide(const nx_String* s) {
    int n = MultiByteToWideChar(65001, 0, (const char*)s->f_data, (int)s->f_len, NULL, 0);
    wchar_t* w = (wchar_t*)nx_alloc(sizeof(wchar_t) * ((size_t)n + 1));
    MultiByteToWideChar(65001, 0, (const char*)s->f_data, (int)s->f_len, w, n);
    w[n] = 0;
    return w;
}
#endif

static NX_UNUSED void nx_print(const nx_String* s, bool newline, bool to_stderr) {
    FILE* f = to_stderr ? stderr : stdout;
    if (s->f_len) fwrite(s->f_data, 1, s->f_len, f);
    if (newline) fputc('\n', f);
    if (to_stderr) fflush(stderr);
}

static NX_UNUSED void nx_flush(void) {
    fflush(stdout);
}

/* Reads one line from standard input, without its line terminator. */
static NX_UNUSED bool nx_read_line(nx_String* out) {
    fflush(stdout);
    size_t cap = 128, n = 0;
    uint8_t* buf = (uint8_t*)nx_alloc(cap);
    int c;
    bool any = false;
    while ((c = fgetc(stdin)) != EOF) {
        any = true;
        if (c == '\n') break;
        if (n + 1 >= cap) {
            cap *= 2;
            buf = (uint8_t*)nx_realloc(buf, cap);
        }
        buf[n++] = (uint8_t)c;
    }
    if (!any) {
        nx_free(buf);
        return false;
    }
    if (n && buf[n - 1] == '\r') n--;
    *out = nx_str_from_bytes_lossy(buf, n);
    nx_free(buf);
    return true;
}

static FILE* nx_fopen(const nx_String* path, const char* mode) {
#if defined(_WIN32)
    wchar_t* w = nx_utf8_to_wide(path);
    wchar_t wmode[8];
    size_t i = 0;
    for (; mode[i] && i < 7; i++) wmode[i] = (wchar_t)mode[i];
    wmode[i] = 0;
    FILE* f = _wfopen(w, wmode);
    nx_free(w);
    return f;
#else
    char* p = (char*)nx_alloc(path->f_len + 1);
    memcpy(p, path->f_data, path->f_len);
    p[path->f_len] = 0;
    FILE* f = fopen(p, mode);
    nx_free(p);
    return f;
#endif
}

/* Returns 0 on success, 1 if the file cannot be opened, 2 if it is not UTF-8. */
static NX_UNUSED int32_t nx_read_file(const nx_String* path, nx_String* out) {
    FILE* f = nx_fopen(path, "rb");
    if (!f) return 1;
    size_t cap = 4096, n = 0;
    uint8_t* buf = (uint8_t*)nx_alloc(cap);
    size_t r;
    while ((r = fread(buf + n, 1, cap - n, f)) > 0) {
        n += r;
        if (n == cap) {
            cap *= 2;
            buf = (uint8_t*)nx_realloc(buf, cap);
        }
    }
    bool err = ferror(f) != 0;
    fclose(f);
    if (err) {
        nx_free(buf);
        return 1;
    }
    if (!nx_utf8_valid(buf, n)) {
        nx_free(buf);
        return 2;
    }
    out->f_data = buf;
    out->f_len = n;
    out->f_cap = cap;
    return 0;
}

static NX_UNUSED bool nx_write_file(const nx_String* path, const nx_String* content, bool append) {
    FILE* f = nx_fopen(path, append ? "ab" : "wb");
    if (!f) return false;
    bool ok = content->f_len == 0 || fwrite(content->f_data, 1, content->f_len, f) == content->f_len;
    ok = (fclose(f) == 0) && ok;
    return ok;
}

static NX_UNUSED bool nx_env(const nx_String* name, nx_String* out) {
    char* n = (char*)nx_alloc(name->f_len + 1);
    memcpy(n, name->f_data, name->f_len);
    n[name->f_len] = 0;
    const char* v = getenv(n);
    nx_free(n);
    if (!v) return false;
    *out = nx_str_from_bytes_lossy((const uint8_t*)v, strlen(v));
    return true;
}

/* Strict float syntax: [+-] (digits [. digits] | . digits) [(e|E) [+-] digits],
   or inf / infinity / nan (any case). */
static NX_UNUSED bool nx_parse_float(const nx_String* s, double* out) {
    size_t n = s->f_len, i = 0;
    const uint8_t* b = s->f_data;
    if (n == 0 || n > 400) return false;
    char buf[408];
    memcpy(buf, b, n);
    buf[n] = 0;
    if (b[i] == '+' || b[i] == '-') i++;
    const char* rest = buf + i;
    size_t rl = n - i;
    bool special = false;
    if ((rl == 3 && (strncmp(rest, "inf", 3) == 0 || strncmp(rest, "INF", 3) == 0 || strncmp(rest, "Inf", 3) == 0)) ||
        (rl == 8 && (strncmp(rest, "infinity", 8) == 0 || strncmp(rest, "Infinity", 8) == 0 || strncmp(rest, "INFINITY", 8) == 0))) {
        *out = (buf[0] == '-') ? -INFINITY : INFINITY;
        return true;
    }
    if (rl == 3 && (strncmp(rest, "nan", 3) == 0 || strncmp(rest, "NaN", 3) == 0 || strncmp(rest, "NAN", 3) == 0)) {
        *out = NAN;
        return true;
    }
    (void)special;
    size_t digits = 0;
    while (i < n && b[i] >= '0' && b[i] <= '9') { i++; digits++; }
    if (i < n && b[i] == '.') {
        i++;
        while (i < n && b[i] >= '0' && b[i] <= '9') { i++; digits++; }
    }
    if (digits == 0) return false;
    if (i < n && (b[i] == 'e' || b[i] == 'E')) {
        i++;
        if (i < n && (b[i] == '+' || b[i] == '-')) i++;
        size_t ed = 0;
        while (i < n && b[i] >= '0' && b[i] <= '9') { i++; ed++; }
        if (ed == 0) return false;
    }
    if (i != n) return false;
    *out = strtod(buf, NULL);
    return true;
}

static NX_UNUSED int32_t nx_arg_count(void) {
    return nx_argc;
}

static NX_UNUSED nx_String nx_arg(int32_t i) {
    if (i < 0 || i >= nx_argc) nx_panic("argument index out of range", NULL);
    return nx_str_from_bytes_lossy((const uint8_t*)nx_argv[i], strlen(nx_argv[i]));
}

static NX_UNUSED uint64_t nx_monotonic_ns(void) {
#if defined(_WIN32)
    long long c, f;
    QueryPerformanceCounter(&c);
    QueryPerformanceFrequency(&f);
    return (uint64_t)((double)c * 1e9 / (double)f);
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000u + (uint64_t)ts.tv_nsec;
#endif
}

NX_NORETURN static NX_UNUSED void nx_exit(int32_t code) {
    fflush(stdout);
    if (getenv("NEXUL_LEAK_CHECK") && nx_live_allocs != 0) {
        char buf[32];
        size_t n;
        long long v = nx_live_allocs;
        nx_write_err("leak check: ", 12);
        if (v < 0) {
            nx_write_err("-", 1);
            v = -v;
        }
        nx_u64_to_dec((uint64_t)v, buf, &n);
        nx_write_err(buf, n);
        nx_write_err(" allocation(s) not freed\n", strlen(" allocation(s) not freed\n"));
        fflush(stderr);
        exit(code == 0 ? 102 : code);
    }
    fflush(stderr);
    exit(code);
}

static void nx_rt_init(int argc, char** argv) {
    nx_argc = argc;
    nx_argv = argv;
#if defined(_WIN32)
    SetConsoleOutputCP(65001);
    /* Re-read the arguments as UTF-8 (the narrow argv uses the ANSI code page). */
    int wargc = 0;
    wchar_t** wargv = CommandLineToArgvW(GetCommandLineW(), &wargc);
    if (wargv) {
        /* Allocated for the whole program: not counted by the leak check. */
        char** u = (char**)malloc(sizeof(char*) * ((size_t)wargc + 1));
        if (!u) nx_panic("out of memory", NULL);
        for (int i = 0; i < wargc; i++) {
            int n = WideCharToMultiByte(65001, 0, wargv[i], -1, NULL, 0, NULL, NULL);
            u[i] = (char*)malloc((size_t)n);
            if (!u[i]) nx_panic("out of memory", NULL);
            WideCharToMultiByte(65001, 0, wargv[i], -1, u[i], n, NULL, NULL);
        }
        u[wargc] = NULL;
        nx_argc = wargc;
        nx_argv = u;
    }
#endif
}
