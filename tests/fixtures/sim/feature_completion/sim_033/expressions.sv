`timescale 1ns/1ns
// SIM-033 A02: clocking inputs bound to selects, concatenations,
// hierarchical and member selects, computed and real expressions.
module sub;
  logic [7:0] sig = 8'h96;
endmodule

module tb;
  typedef struct packed {
    logic [3:0] hi;
    logic [3:0] lo;
  } pair_t;

  logic clk = 1'b0;
  logic [7:0] v = 8'hA5;
  logic [3:0] a = 4'h3;
  logic [3:0] b = 4'hC;
  pair_t p = '{4'h1, 4'h2};
  int n = 5;
  real r = 1.5;
  shortreal sr = 0.25;
  time t_lo = 0;
  time t_b7 = 0;
  sub u ();

  function automatic int twice(int x);
    return 2 * x;
  endfunction

  clocking cb @(posedge clk);
    input lo = v[3:0];
    input bit7 = v[7];
    input cat = {a, b, v[1:0]};
    input hs = tb.u.sig[7:4];
    input pl = p.lo;
    input pw = p;
    input sum = n + a;
    input dbl = twice(n);
    input rr = r;
    input #0 rz = r * 2.0;
    input #2 rh = r;
    input srr = sr;
  endclocking

  task automatic show;
    $display("%0t lo=%h b7=%b cat=%h/%0d hs=%h pl=%h pw=%h/%h sum=%0d dbl=%0d",
             $time, cb.lo, cb.bit7, cb.cat, $bits(cb.cat), cb.hs, cb.pl, cb.pw,
             cb.pw.hi, cb.sum, cb.dbl);
    $display("%0t rr=%.3f rz=%.3f rh=%.3f sr=%.3f", $time, cb.rr, cb.rz, cb.rh,
             cb.srr);
  endtask

  initial begin
    @(cb.lo) t_lo = $time;
    @(posedge cb.bit7) t_b7 = $time;
  end

  initial begin
    @(cb);
    show();
    @(cb);
    show();
    #1 $display("lo@%0t b7@%0t", t_lo, t_b7);
    $finish;
  end

  initial begin
    #2 v = 8'h3C;
    a = 4'h7;
    r = 2.5;
    u.sig = 8'h5A;
    p.hi = 4'hE;
    n = 7;
    sr = shortreal'(0.75);
    #2 clk = 1'b1;
    v = 8'hFF;
    r = 9.0;
    #1 clk = 1'b0;
    #1 r = 3.25;
    #1 v = 8'h81;
    b = 4'h0;
    #2 clk = 1'b1;
  end
endmodule
