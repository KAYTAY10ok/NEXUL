#include <stdint.h>
#include <stdio.h>
int main(void) {
    double x = 1.0, total = 0.0;
    for (int i = 0; i < 10000000; ++i) {
        x = x * 0.5 + (double)(i % 100) * 0.25;
        total += x;
    }
    printf("%llu\n", (unsigned long long)(uint64_t)total);
    return 0;
}
