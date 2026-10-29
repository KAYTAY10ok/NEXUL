#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

enum { CAPACITY = 64, MESSAGES = 100000 };
static int64_t queue[CAPACITY];
static size_t head, tail, count;
static pthread_mutex_t mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t readable = PTHREAD_COND_INITIALIZER;
static pthread_cond_t writable = PTHREAD_COND_INITIALIZER;

static void *produce(void *unused) {
    (void)unused;
    for (int64_t i = 0; i < MESSAGES; ++i) {
        pthread_mutex_lock(&mutex);
        while (count == CAPACITY) pthread_cond_wait(&writable, &mutex);
        queue[tail] = i;
        tail = (tail + 1) % CAPACITY;
        ++count;
        pthread_cond_signal(&readable);
        pthread_mutex_unlock(&mutex);
    }
    return NULL;
}

int main(void) {
    pthread_t producer;
    if (pthread_create(&producer, NULL, produce, NULL)) return EXIT_FAILURE;
    int64_t sum = 0;
    for (int i = 0; i < MESSAGES; ++i) {
        pthread_mutex_lock(&mutex);
        while (count == 0) pthread_cond_wait(&readable, &mutex);
        sum += queue[head];
        head = (head + 1) % CAPACITY;
        --count;
        pthread_cond_signal(&writable);
        pthread_mutex_unlock(&mutex);
    }
    if (pthread_join(producer, NULL)) return EXIT_FAILURE;
    printf("sent=%d count=%d sum=%lld\n", MESSAGES, MESSAGES, (long long)sum);
    return EXIT_SUCCESS;
}
