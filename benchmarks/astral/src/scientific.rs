use std::hint::black_box;
use std::time::Instant;

struct BenchResult {
    sum_x: f64,
    sum_x2: f64,
    harmonic: f64,
    basel: f64,
    ln2: f64,
    pi_leibniz: f64,
    pi_integral: f64,
    quarter_circle: f64,
    sqrt_integral: f64,
    inverse_sqrt_integral: f64,
    e_approx: f64,
    sin_approx: f64,
    cos_approx: f64,
    position: f64,
    velocity: f64,
    population: f64,
    energy: f64,
    error_squared: f64,
}

#[inline(never)]
fn compute(n: i64) -> BenchResult {
    let nf = n as f64;
    let dt = 1.0 / nf;
    let mut r = BenchResult {
        sum_x: 0.0,
        sum_x2: 0.0,
        harmonic: 0.0,
        basel: 0.0,
        ln2: 0.0,
        pi_leibniz: 0.0,
        pi_integral: 0.0,
        quarter_circle: 0.0,
        sqrt_integral: 0.0,
        inverse_sqrt_integral: 0.0,
        e_approx: 1.0,
        sin_approx: 1.0,
        cos_approx: 1.0,
        position: 1.0,
        velocity: 0.0,
        population: 1.0,
        energy: 0.0,
        error_squared: 0.0,
    };
    let mut term_e = 1.0;
    let mut term_sin = 1.0;
    let mut term_cos = 1.0;

    for i in 1..=n {
        let k = i as f64;
        let x = k * dt;
        let x2 = x * x;
        r.sum_x += x;
        r.sum_x2 += x2;
        r.harmonic += 1.0 / k;
        r.basel += 1.0 / (k * k);
        if i % 2 == 1 {
            r.ln2 += 1.0 / k;
            r.pi_leibniz += 4.0 / (2.0 * k - 1.0);
        } else {
            r.ln2 -= 1.0 / k;
            r.pi_leibniz -= 4.0 / (2.0 * k - 1.0);
        }
        r.pi_integral += 4.0 / (1.0 + x2) * dt;
        r.quarter_circle += 4.0 * (1.0 - x2).sqrt() * dt;
        r.sqrt_integral += x.sqrt() * dt;
        r.inverse_sqrt_integral += 1.0 / (1.0 + x).sqrt() * dt;

        term_e /= k;
        r.e_approx += term_e;
        term_sin = -term_sin / ((2.0 * k) * (2.0 * k + 1.0));
        r.sin_approx += term_sin;
        term_cos = -term_cos / ((2.0 * k - 1.0) * (2.0 * k));
        r.cos_approx += term_cos;

        let acceleration = -4.0 * r.position - 0.1 * r.velocity;
        r.velocity += acceleration * dt;
        r.position += r.velocity * dt;
        r.population -= 0.7 * r.population * dt;
        r.energy += (0.5 * r.velocity * r.velocity + 2.0 * r.position * r.position) * dt;
        let error = x * (x + dt) - x2;
        r.error_squared += error * error;
    }
    r
}

fn emit(name: &str, value: f64) {
    println!("{name}={value:.10}");
}

fn main() {
    let n: i64 = std::env::args().nth(1).expect("usage: scientific <iterations>").parse().expect("iterations must be an integer");
    if n <= 0 {
        eprintln!("iterations must be a positive integer");
        std::process::exit(2);
    }
    let warm_n = n.min(100_000);
    let mut warm_checksum = 0.0;
    for _ in 0..10 {
        let warm = compute(black_box(warm_n));
        warm_checksum += warm.sum_x + warm.harmonic + warm.energy;
    }

    let start = Instant::now();
    let r = compute(black_box(n));
    let elapsed_ns = start.elapsed().as_nanos();
    println!("elapsed_ns={elapsed_ns}");
    println!("warmup_checksum={warm_checksum:.10}");
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
}
