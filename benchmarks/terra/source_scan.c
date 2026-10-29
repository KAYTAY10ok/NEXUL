#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    uint64_t count = 0, checksum = 0;
    for (int round = 0; round < 400; round++) {
        FILE *f = fopen("lib/std/io.nx", "rb");
        if (!f || fseek(f, 0, SEEK_END)) return 1;
        long length = ftell(f);
        if (length < 0 || fseek(f, 0, SEEK_SET)) return 1;
        size_t n = (size_t)length;
        unsigned char *bytes = malloc(n ? n : 1);
        if (!bytes || fread(bytes, 1, n, f) != n || fclose(f)) return 1;
        int in_word = 0;
        for (size_t i = 0; i < n; i++) {
            unsigned char c = bytes[i];
            int word = (c >= 65 && c <= 90) || (c >= 97 && c <= 122) || (c >= 48 && c <= 57) || c == 95;
            if (word && !in_word) count++;
            in_word = word;
            checksum += c;
        }
        free(bytes);
    }
    printf("tokens=%llu checksum=%llu\n", (unsigned long long)count, (unsigned long long)checksum);
    return 0;
}
