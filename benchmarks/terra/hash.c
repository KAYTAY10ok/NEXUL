#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
int main(void) {
    const size_t n = 100000, cap = 262144;
    uint64_t total = 0;
    for (size_t round = 0; round < 5; ++round) {
        size_t *keys = calloc(cap, sizeof *keys);
        uint64_t *values = calloc(cap, sizeof *values);
        if (!keys || !values) return 2;
        for (size_t i = 0; i < n; ++i) {
            size_t key = (i * 48271 + 17) % n + 1, slot = (key * 2654435761ULL) % cap;
            while (keys[slot]) slot = (slot + 1) % cap;
            keys[slot] = key; values[slot] = i + round;
        }
        for (size_t i = 0; i < n; ++i) {
            size_t key = (i * 48271 + 17) % n + 1, slot = (key * 2654435761ULL) % cap;
            while (keys[slot] != key) slot = (slot + 1) % cap;
            total += values[slot];
        }
        free(values); free(keys);
    }
    printf("checksum=%llu\n", (unsigned long long)total);
}
