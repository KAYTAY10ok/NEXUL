#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint64_t vectors(uint64_t n) {
    uint64_t *input = malloc((size_t)n * sizeof(*input));
    uint64_t *output = calloc((size_t)n, sizeof(*output));
    if (!input || !output) {
        free(input);
        free(output);
        fprintf(stderr, "vector allocation failed for %" PRIu64 " elements\n", n);
        exit(3);
    }
    for (uint64_t i = 0; i < n; ++i) input[i] = i % 1000 + 1;
    uint64_t i = 0;
    for (; i + 2 < n; i += 3) {
        const uint64_t a = input[i], b = input[i + 1], c = input[i + 2];
        output[i] = 2 * a + 3 * b + 5 * c;
        output[i + 1] = 7 * a + 11 * b + 13 * c;
        output[i + 2] = 17 * a + 19 * b + 23 * c;
    }
    for (; i < n; ++i) output[i] = input[i];
    uint64_t checksum = 0;
    for (i = 0; i < n; ++i) checksum += output[i];
    free(output);
    free(input);
    return checksum;
}

static uint64_t allocations(uint64_t n) {
    uint64_t checksum = 0;
    for (uint64_t i = 0; i < n; ++i) {
        volatile uint64_t *block = malloc(4 * sizeof(*block));
        if (!block) {
            fprintf(stderr, "allocation failed at iteration %" PRIu64 "\n", i);
            exit(3);
        }
        block[0] = i;
        block[1] = i + 3;
        block[2] = i ^ UINT64_C(0x5a5a);
        block[3] = i * 3 + 7;
        checksum += block[0] + block[1] + block[2] + block[3];
        free((void *)block);
    }
    return checksum;
}

static uint64_t strings(uint64_t n) {
    if (n >= SIZE_MAX) {
        fprintf(stderr, "string is too large\n");
        exit(3);
    }
    volatile unsigned char *text = malloc((size_t)n + 1);
    if (!text) {
        fprintf(stderr, "string allocation failed for %" PRIu64 " bytes\n", n);
        exit(3);
    }
    for (uint64_t i = 0; i < n; ++i) {
        const unsigned char value = (unsigned char)('a' + i % 26);
        text[i] = value;
    }
    text[n] = 0;
    uint64_t checksum = 0;
    for (uint64_t i = 0; i < n; ++i) checksum += text[i];
    const uint64_t length = n;
    free(text);
    if (length != n) {
        fprintf(stderr, "string length mismatch\n");
        exit(3);
    }
    return checksum;
}

static uint64_t calculate(const char *mode, uint64_t n) {
    if (strcmp(mode, "vectors") == 0) return vectors(n);
    if (strcmp(mode, "allocations") == 0) return allocations(n);
    if (strcmp(mode, "strings") == 0) return strings(n);
    fprintf(stderr, "unknown workload: %s\n", mode);
    exit(2);
}

int main(int argc, char **argv) {
    if (argc != 3) {
        fprintf(stderr, "usage: dynamic <vectors|allocations|strings> <iterations>\n");
        return 2;
    }
    char *end = NULL;
    const uint64_t n = strtoull(argv[2], &end, 10);
    if (!end || *end != '\0' || n == 0) {
        fprintf(stderr, "iterations must be a positive integer\n");
        return 2;
    }
    const uint64_t warm_n = n < 10000 ? n : 10000;
    volatile uint64_t warm_checksum = 0;
    for (int i = 0; i < 10; ++i) warm_checksum += calculate(argv[1], warm_n);

    LARGE_INTEGER frequency, start, end_time;
    QueryPerformanceFrequency(&frequency);
    QueryPerformanceCounter(&start);
    const uint64_t checksum = calculate(argv[1], n);
    QueryPerformanceCounter(&end_time);
    const uint64_t elapsed_ns = (uint64_t)((end_time.QuadPart - start.QuadPart) * 1000000000LL / frequency.QuadPart);
    printf("elapsed_ns=%" PRIu64 "\n", elapsed_ns);
    printf("checksum=%" PRIu64 "\n", checksum);
    printf("length=%" PRIu64 "\n", n);
    printf("warmup_checksum=%" PRIu64 "\n", (uint64_t)warm_checksum);
    return 0;
}
