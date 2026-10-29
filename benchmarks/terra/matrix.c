#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
int main(int argc, char **argv) {
    (void)argv;
    const size_t n = 64, rounds = 300u + (size_t)argc - 1;
    uint64_t *a = malloc(n*n*sizeof(*a)), *b = malloc(n*n*sizeof(*b)), *c = calloc(n*n,sizeof(*c));
    if (!a || !b || !c) return 1;
    for (size_t i = 0; i < n*n; i++) { a[i] = i % 17; b[i] = i % 13; }
    for (size_t r = 0; r < rounds; r++)
        for (size_t i = 0; i < n; i++)
            for (size_t j = 0; j < n; j++) {
                uint64_t value = 0;
                for (size_t k = 0; k < n; k++) value += a[i*n+k] * b[k*n+j];
                c[i*n+j] += value;
            }
    uint64_t checksum = 0;
    for (size_t i = 0; i < n*n; i++) checksum += c[i];
    printf("%llu\n", (unsigned long long)checksum);
    free(a); free(b); free(c);
    return 0;
}
