// IEEE 1800-2009 16.15.6.1: a const cast in a procedural concurrent
// assertion saves the value of its expression when the instance is queued;
// every other read of a static variable is sampled. The cases follow the
// a1-a3 and a8 examples of the clause; readme.md derives every line.
`timescale 1ns/1ns
module tb;
  bit clk;
  always #5 clk = ~clk;
  logic [3:0] foo = 4'b1010;
  logic [3:0] bar = 4'b1110;
  int i;
  always @(posedge clk) begin
    for (i = 0; i < 3; i++) begin
      c1: assert property (foo[i]) $display("[%0t] c1 pass", $time);
          else $display("[%0t] c1 fail", $time);
      c2: assert property (foo[const'(i)] && bar[i]) $display("[%0t] c2 pass %0d", $time, const'(i));
          else $display("[%0t] c2 fail %0d", $time, const'(i));
      c3: assert property (foo[const'(i)] && bar[const'(i)]) $display("[%0t] c3 pass %0d", $time, const'(i));
          else $display("[%0t] c3 fail %0d", $time, const'(i));
      c4: assert property (foo[const'(i)])
          else $display("[%0t] c4 fail const=%0d sampled=%0d", $time, const'(i), $sampled(i));
    end
  end
  initial #22 $finish;
endmodule
