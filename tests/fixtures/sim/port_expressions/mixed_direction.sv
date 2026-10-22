// One port whose expression mixes an input and an output (23.2.2.1 Example
// 7): the input part reads its bit of the actual, the output part drives
// its bit.
module m(.p({a, e}));
  input a;
  output e;
  assign e = ~a;
endmodule

module tb;
  wire [1:0] w;
  logic d;
  assign w[1] = d;
  m c1(.p(w));
  initial begin
    d = 0;
    #1 $display("1 w=%b", w);
    d = 1;
    #1 $display("2 w=%b", w);
    $finish;
  end
endmodule
