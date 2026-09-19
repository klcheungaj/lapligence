// llg-test-fixture: G1-34 rtl_composition_gate.
// Fixed unpacked arrays with assignment patterns, whole-array copy and slice
// assignment; packed and unpacked structures with nested member selects; and
// zero-time subroutines with recursion, defaults, output/inout copy-out, ref
// formals and packed-structure arguments.
module tb;
    typedef struct packed {
        logic [7:0] hi;
        logic [7:0] lo;
    } pair_t;

    typedef struct {
        pair_t a;
        pair_t b;
        int    idx;
    } outer_t;

    logic [7:0] src [0:3];
    logic [7:0] dst [0:3];
    logic [7:0] part [0:3];
    outer_t u;
    outer_t v;

    function automatic int fib(input int n);
        if (n < 2) fib = n;
        else fib = fib(n - 1) + fib(n - 2);
    endfunction

    function automatic logic [7:0] mix(input pair_t p, inout pair_t acc, output pair_t o);
        acc.lo = acc.lo + p.lo;
        o.hi = acc.hi;
        o.lo = acc.lo;
        mix = p.hi ^ p.lo;
    endfunction

    function automatic pair_t swap(input pair_t p);
        swap = {p.lo, p.hi};
    endfunction

    task automatic combine(input int a = 7,
                           input int b = a + 1,
                           output int sum,
                           inout int acc);
        sum = a + b;
        acc = acc + a;
    endtask

    task automatic bump(ref int x, input int by = 1);
        x = x + by;
    endtask

    function automatic logic [8:0] sum_a(input logic [7:0] x0,
                                         input logic [7:0] x1,
                                         input logic [7:0] x2,
                                         input logic [7:0] x3);
        sum_a = x0 + x1 + x2 + x3;
    endfunction

    int calls;
    function automatic int next_default();
        calls = calls + 1;
        next_default = 40 + calls;
    endfunction

    task automatic take_default(input int a = next_default(), output int y);
        y = a;
    endtask

    int y;
    int s;
    int c;
    int r;
    logic [7:0] mixed;
    pair_t p;
    pair_t acc;
    pair_t o;
    pair_t sw;

    initial begin
        src = '{8'h10, 8'h20, 8'h30, 8'h40};
        dst = src;
        part = '{8'h00, 8'h00, 8'h00, 8'h00};
        part[1:2] = src[0:1];
        part[3] = src[0];
        $display("arr src=%h%h%h%h dst=%h%h%h%h part=%h%h%h%h",
                 src[0], src[1], src[2], src[3],
                 dst[0], dst[1], dst[2], dst[3],
                 part[0], part[1], part[2], part[3]);
        $display("sum=%0d", sum_a(src[0], src[1], src[2], src[3]));

        u.a = '{hi: 8'hAA, lo: 8'h55};
        u.b = '{hi: 8'h11, lo: 8'h22};
        u.idx = 3;
        v = u;
        $display("struct a=%h%h b=%h%h idx=%0d va=%h%h vb=%h%h",
                 u.a.hi, u.a.lo, u.b.hi, u.b.lo, u.idx,
                 v.a.hi, v.a.lo, v.b.hi, v.b.lo);

        p = '{hi: 8'h10, lo: 8'h05};
        acc = '{hi: 8'h20, lo: 8'h02};
        o = '0;
        mixed = mix(p, acc, o);
        sw = swap(p);
        $display("mix=%02h p=%02h%02h acc=%02h%02h o=%02h%02h sw=%02h%02h",
                 mixed, p.hi, p.lo, acc.hi, acc.lo, o.hi, o.lo, sw.hi, sw.lo);

        calls = 0;
        take_default(, y);
        $display("defaults calls=%0d y=%0d", calls, y);
        take_default(5, y);
        $display("defaults calls=%0d y=%0d", calls, y);

        c = 100;
        combine(, , s, c);
        $display("combine s=%0d c=%0d", s, c);
        combine(5, 6, s, c);
        $display("combine s=%0d c=%0d", s, c);

        r = 10;
        bump(r);
        bump(r, 5);
        $display("ref r=%0d", r);

        $display("fib=%0d", fib(10));
        $finish(0);
    end
endmodule
