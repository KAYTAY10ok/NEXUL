using System.Diagnostics;
using System.Globalization;

static class Program
{
    static ulong Vectors(int n)
    {
        var input = new ulong[n];
        var output = new ulong[n];
        for (var i = 0; i < n; ++i) input[i] = (ulong)(i % 1000 + 1);
        var iVector = 0;
        while (iVector + 2 < n)
        {
            var a = input[iVector];
            var b = input[iVector + 1];
            var c = input[iVector + 2];
            output[iVector] = 2 * a + 3 * b + 5 * c;
            output[iVector + 1] = 7 * a + 11 * b + 13 * c;
            output[iVector + 2] = 17 * a + 19 * b + 23 * c;
            iVector += 3;
        }
        while (iVector < n)
        {
            output[iVector] = input[iVector];
            ++iVector;
        }
        ulong checksum = 0;
        foreach (var value in output) checksum += value;
        return checksum;
    }

    static ulong Allocations(int n)
    {
        ulong checksum = 0;
        for (ulong i = 0; i < (ulong)n; ++i)
        {
            var block = new ulong[4];
            block[0] = i;
            block[1] = i + 3;
            block[2] = i ^ 0x5a5a;
            block[3] = i * 3 + 7;
            checksum += block[0] + block[1] + block[2] + block[3];
            GC.KeepAlive(block);
        }
        return checksum;
    }

    static ulong Strings(int n)
    {
        var text = string.Create(n, n, static (span, length) =>
        {
            for (var i = 0; i < length; ++i) span[i] = (char)('a' + i % 26);
        });
        if (text.Length != n) throw new InvalidOperationException("string length mismatch");
        ulong checksum = 0;
        foreach (var value in text) checksum += value;
        GC.KeepAlive(text);
        return checksum;
    }

    static ulong Calculate(string mode, int n) => mode switch
    {
        "vectors" => Vectors(n),
        "allocations" => Allocations(n),
        "strings" => Strings(n),
        _ => throw new ArgumentException($"unknown workload: {mode}"),
    };

    static int Main(string[] args)
    {
        if (args.Length != 2 || !int.TryParse(args[1], CultureInfo.InvariantCulture, out var n) || n <= 0)
        {
            Console.Error.WriteLine("usage: dynamic <vectors|allocations|strings> <iterations>");
            return 2;
        }

        var warmN = Math.Min(n, 10_000);
        ulong warmupChecksum = 0;
        for (var i = 0; i < 10; ++i) warmupChecksum = unchecked(warmupChecksum + Calculate(args[0], warmN));

        var start = Stopwatch.GetTimestamp();
        var checksum = Calculate(args[0], n);
        var elapsedNs = (ulong)Stopwatch.GetElapsedTime(start).Ticks * 100;
        Console.WriteLine($"elapsed_ns={elapsedNs}");
        Console.WriteLine($"checksum={checksum}");
        Console.WriteLine($"length={n}");
        Console.WriteLine($"warmup_checksum={warmupChecksum}");
        return 0;
    }
}
