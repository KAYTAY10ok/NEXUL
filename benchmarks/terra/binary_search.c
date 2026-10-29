#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
int main(int argc, char **argv) {
    (void)argv;
    const size_t n = 65536, count = 1000000u + (size_t)argc - 1;
    uint64_t *values = malloc(n * sizeof(*values));
    if (!values) return 1;
    for (size_t i = 0; i < n; i++) values[i] = i * 3;
    uint64_t x = 7, checksum = 0;
    for (size_t i = 0; i < count; i++) {
        x = (x * 48271 + 17) % 2147483647;
        uint64_t key = x % 196608;
        size_t lo = 0, hi = n;
        while (lo < hi) {
            size_t mid = lo + (hi - lo) / 2;
            if (values[mid] < key) lo = mid + 1; else hi = mid;
        }
        checksum += lo;
    }
    printf("%llu\n", (unsigned long long)checksum);
    free(values);
    return 0;
}
