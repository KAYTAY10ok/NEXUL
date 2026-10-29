#include <stdint.h>
#include <stdio.h>
int main(int argc, char **argv) {
    (void)argv;
    size_t count = 20000000u + (size_t)argc - 1;
    uint64_t x = 7, sum = 0;
    for (size_t i = 0; i < count; i++) {
        x = (x * 48271 + 17) % 2147483647;
        sum += x % 1009;
    }
    printf("%llu\n", (unsigned long long)sum);
    return 0;
}
