// ADV-013 A02/A03: local variable formals and declared locals in recursive
// and multiclocked properties: mutual recursion, actuals swapped by value,
// an if condition and a declaration assignment reading a formal, an
// antecedent local read through recursion, two leading clocks under `and`
// and under `not (... or ...)`, recursion under `nexttime`. Expected lines
// are derived in readme.md.
`timescale 1ns/1ns
module tb;
  logic clk = 0, c2 = 0, s = 0;
  logic [3:0] d = 0, e = 0;
  logic [2:0] x = 0;
  always #5 clk = ~clk;
  initial begin #7 forever #10 c2 = ~c2; end   // c2 rises at 17, 37, 57, ...
  // mutual recursion with local formals: d counts up from k, then e counts down
  property up(local input logic [3:0] k);
    (d == k) and (1'b1 |=> down(k + 4'd1));
  endproperty
  property down(local input logic [3:0] k);
    (e == k) and (1'b1 |=> up(k + 4'd1));
  endproperty
  // swap by value through temporaries
  property sw(local input logic [3:0] p, q);
    (d == p) and (1'b1 |=> sw(q, p));
  endproperty
  // local if and declared local initialized from a formal
  property cs(local input logic [3:0] k);
    logic [3:0] m = k + 4'd1;
    if (k == 4'd3) (d + 4'd1 == m) else if (k == 4'd9) (e == m) else (d == 4'd15);
  endproperty
  // an antecedent local read through recursion
  property keep(local input int n, logic [3:0] v);
    (n > 0) |-> ((d != v) and (1'b1 |=> keep(n - 1, v)));
  endproperty
  property outer;
    logic [3:0] v;
    (s, v = e) |=> keep(3, v);
  endproperty
  // a local formal with two semantic leading clocks
  property tc(local input logic [2:0] v);
    (@(posedge clk) v == 3'd4) and (@(posedge c2) v == 3'd3);
  endproperty
  // not over the two copies
  property ntc(local input logic [2:0] v);
    not ((@(posedge clk) v == 3'd1) or (@(posedge c2) v == 3'd2));
  endproperty
  // recursion under always with a local formal
  property al(local input logic [3:0] k);
    nexttime ((e != k) and (1'b1 |=> al(k + 4'd2)));
  endproperty

  m1: assert property (@(posedge clk) s |-> up(d)) $display("m1 P %0t", $time); else $display("m1 F %0t", $time);
  m2: assert property (@(posedge clk) s |-> sw(d, e)) $display("m2 P %0t", $time); else $display("m2 F %0t", $time);
  m3: assert property (@(posedge clk) s |=> cs(d)) $display("m3 P %0t", $time); else $display("m3 F %0t", $time);
  m4: assert property (@(posedge clk) outer) $display("m4 P %0t", $time); else $display("m4 F %0t", $time);
  m5: assert property (@(posedge clk) s |=> tc(x)) $display("m5 P %0t", $time); else $display("m5 F %0t", $time);
  m6: assert property (@(posedge clk) s |=> ntc(x)) $display("m6 P %0t", $time); else $display("m6 F %0t", $time);
  m7: assert property (@(posedge clk) s |-> al(e)) $display("m7 P %0t", $time); else $display("m7 F %0t", $time);
  initial begin
    #12 s = 1'b1; d = 4'd2; e = 4'd9; x = 3'd3;   // tick 15
    #6 x = 3'd4;                               // 18 (c2 17 sampled 3; clk 25 samples 4)
    #4 s = 1'b0; d = 4'd3; e = 4'd3;              // 22: tick 25
    #10 d = 4'd4; e = 4'd4;                    // 32: tick 35
    #10 d = 4'd5; e = 4'd5;                    // 42: tick 45
    #10 d = 4'd6; e = 4'd7; s = 1'b1; x = 3'd2;   // 52: tick 55
    #10 s = 1'b0; d = 4'd9; e = 4'd8; x = 3'd1;   // 62: tick 65
    #10 d = 4'd8; e = 4'd8;                    // 72: tick 75
    #10 d = 4'd9; e = 4'd10;                   // 82: tick 85
    #20 $finish;
  end
endmodule
