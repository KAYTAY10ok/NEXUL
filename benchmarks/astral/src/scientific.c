#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <inttypes.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

typedef struct {
    double sum_x, sum_x2, harmonic, basel, ln2, pi_leibniz;
    double pi_integral, quarter_circle, sqrt_integral, inverse_sqrt_integral;
    double e_approx, sin_approx, cos_approx, position, velocity, population;
    double energy, error_squared;
} bench_result;

__declspec(noinline) static bench_result compute(int64_t n) {
    const double nf = (double)n;
    const double dt = 1.0 / nf;
    bench_result r = {0};
    double term_e = 1.0, term_sin = 1.0, term_cos = 1.0;
    double velocity = 0.0, population = 1.0;
    r.e_approx = 1.0;
    r.sin_approx = 1.0;
    r.cos_approx = 1.0;
    r.position = 1.0;

    for (int64_t i = 1; i <= n; ++i) {
        const double k = (double)i;
        const double x = k * dt;
        const double x2 = x * x;
        r.sum_x += x;
        r.sum_x2 += x2;
        r.harmonic += 1.0 / k;
        r.basel += 1.0 / (k * k);
        if ((i & 1) != 0) {
            r.ln2 += 1.0 / k;
            r.pi_leibniz += 4.0 / (2.0 * k - 1.0);
        } else {
            r.ln2 -= 1.0 / k;
            r.pi_leibniz -= 4.0 / (2.0 * k - 1.0);
        }
        r.pi_integral += 4.0 / (1.0 + x2) * dt;
        r.quarter_circle += 4.0 * sqrt(1.0 - x2) * dt;
        r.sqrt_integral += sqrt(x) * dt;
        r.inverse_sqrt_integral += 1.0 / sqrt(1.0 + x) * dt;

        term_e /= k;
        r.e_approx += term_e;
        term_sin = -term_sin / ((2.0 * k) * (2.0 * k + 1.0));
        r.sin_approx += term_sin;
        term_cos = -term_cos / ((2.0 * k - 1.0) * (2.0 * k));
        r.cos_approx += term_cos;

        const double acceleration = -4.0 * r.position - 0.1 * velocity;
        velocity += acceleration * dt;
        r.position += velocity * dt;
        population -= 0.7 * population * dt;
        r.energy += (0.5 * velocity * velocity + 2.0 * r.position * r.position) * dt;
        const double error = x * (x + dt) - x2;
        r.error_squared += error * error;
    }
    r.velocity = velocity;
    r.population = population;
    return r;
}

static void emit(const char *name, double value) { printf("%s=%.10f\n", name, value); }

int main(int argc, char **argv) {
    if (argc != 2) {
        fprintf(stderr, "usage: scientific <iterations>\n");
        return 2;
    }
    char *end = NULL;
    const int64_t n = strtoll(argv[1], &end, 10);
    if (!end || *end != '\0' || n <= 0) {
        fprintf(stderr, "iterations must be a positive integer\n");
        return 2;
    }

    int64_t warm_n = n < 100000 ? n : 100000;
    volatile double warm_checksum = 0.0;
    for (int i = 0; i < 10; ++i) {
        const bench_result warm = compute(warm_n);
        warm_checksum += warm.sum_x + warm.harmonic + warm.energy;
    }

    LARGE_INTEGER frequency, start, end_time;
    QueryPerformanceFrequency(&frequency);
    QueryPerformanceCounter(&start);
    const bench_result r = compute(n);
    QueryPerformanceCounter(&end_time);
    const uint64_t elapsed_ns = (uint64_t)((end_time.QuadPart - start.QuadPart) * 1000000000LL / frequency.QuadPart);
    printf("elapsed_ns=%" PRIu64 "\n", elapsed_ns);
    printf("warmup_checksum=%.10f\n", (double)warm_checksum);
    emit("sum_x", r.sum_x);
    emit("sum_x2", r.sum_x2);
    emit("harmonic", r.harmonic);
    emit("basel", r.basel);
    emit("ln2", r.ln2);
    emit("pi_leibniz", r.pi_leibniz);
    emit("pi_integral", r.pi_integral);
    emit("quarter_circle", r.quarter_circle);
    emit("sqrt_integral", r.sqrt_integral);
    emit("inverse_sqrt_integral", r.inverse_sqrt_integral);
    emit("e_approx", r.e_approx);
    emit("sin_approx", r.sin_approx);
    emit("cos_approx", r.cos_approx);
    emit("position", r.position);
    emit("velocity", r.velocity);
    emit("population", r.population);
    emit("energy", r.energy);
    emit("error_squared", r.error_squared);
    return 0;
}
