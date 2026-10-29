#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
int main(void) {
    const size_t n = 100000;
    uint64_t total = 0;
    for (size_t round = 0; round < 4; ++round) {
        uint64_t *data = malloc(n * sizeof *data), *tmp = malloc(n * sizeof *tmp);
        if (!data || !tmp) return 2;
        for (size_t i = 0; i < n; ++i) data[i] = (i * 48271 + 17 + round) % n;
        for (size_t width = 1; width < n; width *= 2) {
            for (size_t start = 0; start < n;) {
                size_t mid = start + width < n ? start + width : n;
                size_t end = start + 2 * width < n ? start + 2 * width : n;
                size_t a = start, b = mid;
                for (size_t out = start; out < end; ++out)
                    tmp[out] = b == end || (a < mid && data[a] <= data[b]) ? data[a++] : data[b++];
                start = end;
            }
            for (size_t i = 0; i < n; ++i) data[i] = tmp[i];
        }
        for (size_t i = 0; i < n; ++i) total += data[i] * (i % 17 + 1);
        free(tmp); free(data);
    }
    printf("checksum=%llu\n", (unsigned long long)total);
}
