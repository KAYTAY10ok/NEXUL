/* The C side of tests/interop: calls NEXUL, which calls back into C. */
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>

int32_t nx_add(int32_t a, int32_t b);
int32_t nx_apply(int32_t (*f)(int32_t), int32_t x);
size_t nx_words(const char *text);
int32_t nx_choose(bool first, int32_t a, int32_t b);
void nx_hello(int32_t n);

int32_t c_twice(int32_t x) { return 2 * x; }

static int32_t square(int32_t x) { return x * x; }

int main(void) {
    printf("add %d\n", nx_add(2, 40));
    printf("apply %d\n", nx_apply(square, 7));
    printf("words %d\n", (int)nx_words("  le compilateur  NEXUL appelle C "));
    printf("choose %d %d\n", nx_choose(true, 1, 2), nx_choose(false, 1, 2));
    fflush(stdout);
    for (int i = 0; i < 3; i++) {
        nx_hello(i);
    }
    printf("back in C\n");
    return 0;
}
