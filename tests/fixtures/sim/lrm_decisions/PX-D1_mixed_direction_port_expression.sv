// Decision PX-D1: one port whose port expression concatenates an input and
// an output is legal. Each part connects to its bits of the actual in its
// own direction: the input part reads them, the output part drives them.
//
// IEEE 1800-2009 23.2.2.1 (SystemVerilog-1800-2009.txt L40589-40593):
//   "Example 7-Explicitly named port with mix of input and output directions
//   (non-ANSI style module header)
//         module mixed_direction (.p({a, e}));
//         input a;              // p contains both input and output directions.
//         output e;"
module mixed_direction(.p({a, e}));
  input a;
  output e;
  assign e = ~a;
endmodule

module tb;
  wire [1:0] w;
  logic d;
  assign w[1] = d;
  mixed_direction c1(.p(w));
  initial begin
    d = 0;
    #1 $display("d=0 w=%b", w);
    d = 1;
    #1 $display("d=1 w=%b", w);
    $finish;
  end
endmodule
