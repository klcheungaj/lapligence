// SV2009 11.4.1-11.4.2 (operator assignments, increment/decrement), 13.4
// (functions), 10.3 (continuous assignments), 23.3 (ports), 6.5: each
// receiver, index and right-hand-side call runs exactly once per mutation,
// the stored value is the operator result at the target's width and sign,
// and prefix/postfix expressions yield the new/old value. Function bodies
// called from a continuous assignment are procedural statements, so their
// writes to module variables are legal (6.5 restricts only the target).
interface regs_if;
  logic [128:0] r [0:3];
endinterface

module child(input logic [128:0] i, output logic [128:0] o);
  logic [128:0] mem [0:1];
  assign o = i * 3;
endmodule

module tb;
  typedef struct packed { logic [64:0] lo; logic signed [63:0] hi; } pair_t;
  typedef struct { logic [7:0] f [0:2]; pair_t p; } rec_t;

  int ci, cr, cc;
  int k;
  logic [128:0] mem [0:3];
  logic [7:0] b8 [0:3];
  logic [128:0] y;
  logic signed [64:0] s65;
  logic [7:0] a8;
  logic signed [7:0] s8;
  logic signed [3:0] sn4;
  logic [64:0] w65;
  rec_t rs [0:1];
  regs_if bus();
  logic [128:0] cin, cy, pa, pb, po1, po2;
  logic [128:0] outs [0:1];

  function automatic int idx(int v);
    ci++;
    return v;
  endfunction

  function automatic logic [128:0] rhs(logic [128:0] v);
    cr++;
    return v;
  endfunction

  function automatic logic [128:0] tick(logic [128:0] v);
    cc++;
    return v;
  endfunction

  // Automatic function: mutations of locals through call-valued indices.
  function automatic logic [128:0] fmut(logic [128:0] a);
    logic [128:0] loc [0:1];
    loc[0] = a;
    loc[1] = 0;
    loc[idx(0)] *= rhs(a);
    loc[idx(1)]++;
    return loc[0] + loc[1]++;
  endfunction

  // Only continuous assignments and ports call this one, so `cc` counts
  // their evaluations.
  function automatic logic [128:0] fcont(logic [128:0] a);
    logic [128:0] t;
    t = a;
    t <<= tick(1);
    t -= tick(129'd1);
    return ++t;
  endfunction

  // Static function: its local keeps state between calls.
  function logic [128:0] fstat(logic [128:0] a);
    logic [128:0] acc = 0;
    acc += a;
    return acc;
  endfunction

  assign cy = fcont(cin);
  child u(.i(fcont(pa) + pb), .o(po1));
  child u2(.i(pa * pb), .o(outs[1]));

  initial begin
    // Procedural context.
    mem[1] = 5;
    mem[idx(1)] *= rhs(7);
    $display("A %0d %0d %0d", mem[1], ci, cr);
    y = mem[idx(1)]++ + 1;
    $display("B %0d %0d %0d", y, mem[1], ci);
    y = ++mem[idx(1)] * 2;
    $display("C %0d %0d %0d", y, mem[1], ci);
    mem[2] = 9;
    mem[idx(2)] /= rhs(0);
    $display("D %h %0d %0d", mem[2], ci, cr);
    k = 0;
    mem[0] = 10;
    mem[idx(k++)] -= rhs(1);
    $display("E %0d %0d %0d %0d", mem[0], k, ci, cr);
    b8[0] = 3;
    mem[3] = '1;
    mem[b8[idx(0)]++] += 1;
    $display("F %h %0d %0d", mem[3], b8[0], ci);
    rs[1].p = '0;
    rs[1].p.lo = 1;
    rs[idx(1)].p.lo <<= rhs(64);
    rs[idx(1)].p.hi -= rhs(1);
    $display("G %h %0d %0d", rs[1].p, ci, cr);
    rs[0].f[2] = 8'd250;
    y = rs[idx(0)].f[idx(2)]++;
    $display("H %0d %0d %0d", y, rs[0].f[2], ci);
    y = (rs[idx(0)].f[idx(2)] += 8'd10);
    $display("I %0d %0d %0d", y, rs[0].f[2], ci);
    bus.r[2] = 129'h1_0000_0000_0000_0000;
    bus.r[idx(2)] *= rhs(129'h1_0000_0000_0000_0000);
    u.mem[idx(1)] = 129'd7;
    u.mem[idx(1)] %= rhs(129'd4);
    $display("J %h %0d %0d %0d", bus.r[2], u.mem[1], ci, cr);
    s65 = 65'sh1_0000_0000_0000_0000;
    s65--;
    $display("K %h", s65);
    s65 = 65'sh1_0000_0000_0000_0000;
    s65 /= -1;
    $display("L %h", s65);
    a8 = 200;
    a8 += 16'd100;
    s8 = -7;
    s8 /= 8'd2;
    $display("M %0d %0d", a8, s8);
    s8 = -7;
    s8 /= 2;
    $display("N %0d", s8);
    s8 = -7;
    s8 %= 2;
    $display("O %0d", s8);
    sn4 = -8;
    sn4 >>>= 1;
    $display("P %h", sn4);
    sn4 >>>= 100;
    $display("Q %h", sn4);
    w65 = '1;
    w65 += 1;
    $display("R %h", w65);
    w65 = 'x;
    w65++;
    $display("S %h", w65);

    // Function context.
    ci = 0;
    cr = 0;
    y = fmut(129'h1_0000_0000_0000_0001);
    $display("T %h %0d %0d", y, ci, cr);
    y = fstat(5);
    y = fstat(6);
    $display("U %0d", y);

    // Continuous and port contexts: one evaluation per operand change.
    #1;
    cc = 0;
    cin = 129'h1_0000_0000_0000_0000_0000_0000_0000_0001;
    #1;
    $display("V %h %0d", cy, cc);
    cc = 0;
    pa = 129'h1_0000_0000_0000_0000_0000_0000;
    pb = 3;
    #1;
    $display("W %h %h %0d", po1, outs[1], cc);
    $finish(0);
  end
endmodule
