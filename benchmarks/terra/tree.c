#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
int main(void) {
    const size_t n = 50000;
    uint64_t total = 0;
    for (size_t round = 0; round < 4; ++round) {
        size_t *keys = calloc(n + 1, sizeof *keys), *left = calloc(n + 1, sizeof *left), *right = calloc(n + 1, sizeof *right);
        if (!keys || !left || !right) return 2;
        for (size_t i = 0; i < n; ++i) {
            size_t key = (i * 48271 + 17) % n + 1, node = i + 1;
            keys[node] = key;
            if (node > 1) {
                size_t cur = 1;
                for (;;) {
                    size_t *child = key < keys[cur] ? &left[cur] : &right[cur];
                    if (!*child) { *child = node; break; }
                    cur = *child;
                }
            }
        }
        for (size_t i = 0; i < n; ++i) {
            size_t key = (i * 48271 + 17) % n + 1, cur = 1;
            while (keys[cur] != key) cur = key < keys[cur] ? left[cur] : right[cur];
            total += cur;
        }
        free(right); free(left); free(keys);
    }
    printf("checksum=%llu\n", (unsigned long long)total);
}
