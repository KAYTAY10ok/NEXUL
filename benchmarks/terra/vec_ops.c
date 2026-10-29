#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
int main(void) {
    const size_t n = 4000000;
    uint64_t *values = malloc(n * sizeof(*values));
    if (!values) return 1;
    for (size_t i = 0; i < n; ++i) values[i] = i % 1000;
    for (int pass = 0; pass < 3; ++pass)
        for (size_t i = 0; i < n; ++i) values[i] = values[i] * 3 + 7;
    uint64_t sum = 0;
    for (size_t i = 0; i < n; ++i) sum += values[i];
    printf("%llu\n", (unsigned long long)sum);
    free(values);
    return 0;
}
