// An application acts in a callback of every region and reason.
`timescale 1ns/1ns
module leaf(input logic a, output logic y);
  assign #1 y = ~a;
endmodule
module tb;
  logic clk = 0, a = 0, y;
  int v = 0;
  logic [7:0] q = 0;
  int dyn[];
  int steps;
  leaf u(a, y);
  always #5 clk = ~clk;
  always @(posedge clk) q <= q + 1;
  always @(posedge clk) a <= ~a;
  task automatic work(input int n);
    begin : inner
      if (n > 0) work(n - 1);
      else begin steps++; #1 steps++; end
    end
  endtask
  initial begin
    dyn = new[4];
    $arm;
    #2 v = 1;
    #10 force v = 5;
    #5 release v;
    #5 disable w;
    #3 work(3);
    v = $fn(3) + $fn(4);
    #10 $display("[%0t] main ends steps=%0d q=%0d", $time, steps, q);
    $finish;
  end
  initial begin : w
    #1000 $display("never");
  end
  initial #60 $display("late");
  final begin $display("[%0t] final q=%0d", $time, q); $arm; end
endmodule
