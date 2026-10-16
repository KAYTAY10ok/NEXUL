using System.Diagnostics;
using System.Globalization;
using System.Runtime.CompilerServices;

struct BenchResult
{
    public double SumX, SumX2, Harmonic, Basel, Ln2, PiLeibniz;
    public double PiIntegral, QuarterCircle, SqrtIntegral, InverseSqrtIntegral;
    public double EApprox, SinApprox, CosApprox, Position, Velocity, Population;
    public double Energy, ErrorSquared;
}

static class Program
{
    [MethodImpl(MethodImplOptions.NoInlining)]
    static BenchResult Compute(long n)
    {
        double nf = n;
        double dt = 1.0 / nf;
        var r = new BenchResult { EApprox = 1.0, SinApprox = 1.0, CosApprox = 1.0, Position = 1.0, Population = 1.0 };
        double termE = 1.0, termSin = 1.0, termCos = 1.0;

        for (long i = 1; i <= n; ++i)
        {
            double k = i;
            double x = k * dt;
            double x2 = x * x;
            r.SumX += x;
            r.SumX2 += x2;
            r.Harmonic += 1.0 / k;
            r.Basel += 1.0 / (k * k);
            if ((i & 1) != 0)
            {
                r.Ln2 += 1.0 / k;
                r.PiLeibniz += 4.0 / (2.0 * k - 1.0);
            }
            else
            {
                r.Ln2 -= 1.0 / k;
                r.PiLeibniz -= 4.0 / (2.0 * k - 1.0);
            }
            r.PiIntegral += 4.0 / (1.0 + x2) * dt;
            r.QuarterCircle += 4.0 * Math.Sqrt(1.0 - x2) * dt;
            r.SqrtIntegral += Math.Sqrt(x) * dt;
            r.InverseSqrtIntegral += 1.0 / Math.Sqrt(1.0 + x) * dt;

            termE /= k;
            r.EApprox += termE;
            termSin = -termSin / ((2.0 * k) * (2.0 * k + 1.0));
            r.SinApprox += termSin;
            termCos = -termCos / ((2.0 * k - 1.0) * (2.0 * k));
            r.CosApprox += termCos;

            double acceleration = -4.0 * r.Position - 0.1 * r.Velocity;
            r.Velocity += acceleration * dt;
            r.Position += r.Velocity * dt;
            r.Population -= 0.7 * r.Population * dt;
            r.Energy += (0.5 * r.Velocity * r.Velocity + 2.0 * r.Position * r.Position) * dt;
            double error = x * (x + dt) - x2;
            r.ErrorSquared += error * error;
        }
        return r;
    }

    static void Emit(string name, double value) => Console.WriteLine($"{name}={value.ToString("F10", CultureInfo.InvariantCulture)}");

    static int Main(string[] args)
    {
        if (args.Length != 1 || !long.TryParse(args[0], NumberStyles.None, CultureInfo.InvariantCulture, out long n) || n <= 0)
        {
            Console.Error.WriteLine("usage: scientific <positive iterations>");
            return 2;
        }
        long warmN = Math.Min(n, 100_000);
        double warmChecksum = 0.0;
        for (int i = 0; i < 10; ++i)
        {
            var warm = Compute(warmN);
            warmChecksum += warm.SumX + warm.Harmonic + warm.Energy;
            GC.KeepAlive(warm);
        }

        long start = Stopwatch.GetTimestamp();
        var r = Compute(n);
        long elapsedNs = (long)Stopwatch.GetElapsedTime(start).TotalNanoseconds;
        Console.WriteLine($"elapsed_ns={elapsedNs}");
        Console.WriteLine($"warmup_checksum={warmChecksum.ToString("F10", CultureInfo.InvariantCulture)}");
        Emit("sum_x", r.SumX);
        Emit("sum_x2", r.SumX2);
        Emit("harmonic", r.Harmonic);
        Emit("basel", r.Basel);
        Emit("ln2", r.Ln2);
        Emit("pi_leibniz", r.PiLeibniz);
        Emit("pi_integral", r.PiIntegral);
        Emit("quarter_circle", r.QuarterCircle);
        Emit("sqrt_integral", r.SqrtIntegral);
        Emit("inverse_sqrt_integral", r.InverseSqrtIntegral);
        Emit("e_approx", r.EApprox);
        Emit("sin_approx", r.SinApprox);
        Emit("cos_approx", r.CosApprox);
        Emit("position", r.Position);
        Emit("velocity", r.Velocity);
        Emit("population", r.Population);
        Emit("energy", r.Energy);
        Emit("error_squared", r.ErrorSquared);
        return 0;
    }
}
