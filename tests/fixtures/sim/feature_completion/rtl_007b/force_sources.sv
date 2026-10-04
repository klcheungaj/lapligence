// Effectful helpers as force sources (SV 10.6, 10.6.2).
module tb;
  logic [7:0] x = 8'd1, s = 8'd3, y, z, hi, lo;
  wire [7:0] w = x;
  real rsrc = 1.5, ry;
  int cnt = 0, other_cnt = 0, rcnt = 0;
  int big [0:65536];
  int k = 5, q;

  // Visible write.
  function automatic logic [7:0] bump(input logic [7:0] v);
    cnt++;
    return v + 8'd1;
  endfunction

  function automatic logic [7:0] other(input logic [7:0] v);
    other_cnt++;
    return v + 8'd100;
  endfunction

  // Persistent static result: keeps the last positive argument.
  function logic [7:0] keep(input logic [7:0] v);
    if (v != 0) keep = v;
  endfunction

  // Persistent static local whose result does not depend on how often it ran.
  function logic [15:0] wide(input logic [7:0] v);
    static logic [7:0] last = 0;
    last = v;
    return {v, ~v};
  endfunction

  function automatic real half(input real v);
    rcnt++;
    return v / 2.0;
  endfunction

  // Descriptor-transported array formal.
  function automatic int pick(input int a [0:65536], input int i);
    return a[i] + a[65536];
  endfunction

  initial begin
    big[5] = 10;
    big[65536] = 1;
    force y = bump(x);
    $display("t0 y=%0d cnt_ok=%0d", y, cnt >= 1);
    #1 x = 8'd5;
    #1 $display("t2 y=%0d", y);
    // Replacing the force stops the first site's re-evaluation.
    force y = other(x);
    cnt = 0;
    #1 x = 8'd6;
    #1 $display("t4 y=%0d cnt=%0d other_ok=%0d", y, cnt, other_cnt >= 2);
    release y;
    other_cnt = 0;
    #1 x = 8'd7;
    #1 $display("t6 y=%0d other_cnt=%0d", y, other_cnt);
    force w = bump(x);
    #1 $display("t7 w=%0d", w);
    release w;
    #1 $display("t8 w=%0d", w);
    force z = keep(s);
    #1 s = 8'd0;
    #1 $display("t10 z=%0d", z);
    s = 8'd9;
    #1 $display("t11 z=%0d", z);
    force {hi, lo} = wide(x);
    #1 $display("t12 hi=%0d lo=%0d last=%0d", hi, lo, wide.last);
    x = 8'd2;
    #1 $display("t13 hi=%0d lo=%0d last=%0d", hi, lo, wide.last);
    release lo;
    x = 8'd3;
    #1 $display("t14 hi=%0d lo=%0d", hi, lo);
    force ry = half(rsrc);
    #1 rsrc = 3.0;
    #1 $display("t16 ry=%0.2f rcnt_ok=%0d", ry, rcnt >= 2);
    force q = pick(big, k);
    #1 $display("t17 q=%0d", q);
    big[65536] = 4;
    #1 $display("t18 q=%0d", q);
    k = 6;
    #1 $display("t19 q=%0d", q);
    $finish(0);
  end
endmodule
