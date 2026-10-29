#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
int main(void) {
    const size_t n = 5000000;
    char *text = malloc(n);
    if (!text) return 1;
    for (size_t i = 0; i < n; ++i) text[i] = (char)('a' + i % 26);
    uint64_t sum = 0;
    for (size_t i = 0; i < n; ++i) sum += (unsigned char)text[i];
    printf("%llu\n", (unsigned long long)sum);
    free(text);
    return 0;
}
