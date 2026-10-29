#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int main(void) {
    char dir[] = "/tmp/nexul-bench-XXXXXX";
    if (!mkdtemp(dir)) return 1;
    char path[128];
    if (snprintf(path, sizeof path, "%s/data", dir) < 0) return 1;
    unsigned char bytes[65536], data[65536];
    for (size_t i = 0; i < sizeof bytes; i++) bytes[i] = (unsigned char)(i % 251);
    uint64_t sum = 0;
    for (int round = 0; round < 128; round++) {
        FILE *f = fopen(path, "wb");
        if (!f || fwrite(bytes, 1, sizeof bytes, f) != sizeof bytes || fclose(f)) return 1;
        f = fopen(path, "rb");
        if (!f || fread(data, 1, sizeof data, f) != sizeof data || fclose(f)) return 1;
        for (size_t i = 0; i < sizeof data; i++) sum += data[i];
    }
    if (remove(path) || rmdir(dir)) return 1;
    printf("checksum=%llu\n", (unsigned long long)sum);
    return 0;
}
