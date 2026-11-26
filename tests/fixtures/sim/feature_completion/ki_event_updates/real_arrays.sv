// Event-controlled and delayed nonblocking writes of whole real arrays and
// their elements, and an `iff` qualifier that reads `.triggered`.
`timescale 1ns / 1ns
module tb;
  real reals[2];
  real r2[1:3];
  real r3[2];
  event ev;
  int i = 1;
  int x, y;
  logic clk;

  initial begin : issue
    reals <= @(ev) '{1.5, 2.5};
    r2 <= #1 '{3.5, 4.5, 5.5};
    r2[i + 1] <= #2 9.25;
    r3[i] <= @(ev) 7.5;
    r3[0] <= repeat (2) @(ev) 0.25;
    x <= @(posedge clk iff ev.triggered) 5;
  end

  initial begin : wait_triggered
    @(posedge clk iff ev.triggered);
    y <= 6;
  end

  initial begin : stimulus
    clk = 1'b0;
    #0 $display("%p %p %p", reals, r2, r3);
    #1 clk = 1'b1;
    $display("%p %p %p %0d %0d", reals, r2, r3, x, y);
    #1 clk = 1'b0;
    -> ev;
    #1 $display("%p %p %p %0d %0d", reals, r2, r3, x, y);
    -> ev;
    clk = 1'b1;
    #1 $display("%p %p %p %0d %0d", reals, r2, r3, x, y);
    $finish;
  end
endmodule
