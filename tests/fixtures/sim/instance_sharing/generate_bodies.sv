`timescale 1ns/100ps
// Each iteration of the loop generate construct holds a function, a task, an
// inertial continuous assignment to one bit of a shared net and an initial
// process; only genvar payloads and driver identities differ per iteration.
module tb;
  reg [7:0] a = 8'b0000_0000;
  wire [7:0] y;
  reg [7:0] q = 8'b0000_0000;
  for (genvar i = 0; i < 8; i++) begin : g
    integer sum;
    function automatic integer scale(input integer x);
      scale = x * 10 + i;
    endfunction
    task automatic hold(input integer d);
      #(d) q[i] = a[i];
    endtask
    assign #(i + 1) y[i] = ~a[i];
    initial begin
      sum = scale(i);
      #21 hold(i);
    end
  end
  initial begin
    #20 a = 8'b1010_0110;
    #4.5 $display("y=%b", y);
    #15.5 $display("y=%b q=%b", y, q);
    $display("sum %0d %0d %0d", g[0].sum, g[3].sum, g[7].sum);
    $finish(0);
  end
endmodule
