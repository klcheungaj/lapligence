// Decision S23-D7: a bit-select or part-select of an element of an unpacked
// array is not a force or release target.
//
// IEEE 1800-2009 10.6.2 (SystemVerilog-1800-2009.txt L13373-13376):
//   "The left-hand side of the assignment can be a reference to a singular
//   variable, a net, a constant bit-select of a vector net, a constant
//   part-select of a vector net, or a concatenation of these. It shall not be
//   a bit-select or a part-select of a variable."
//
// llg rejects the design at compile time with the target location. Expected
// result: the design is rejected; it prints nothing (the .out file is
// empty). Industry-standard behaviour forces the selected bits and would
// print "1 2f", "2 3f", "3 3f".
module tb;
  logic [7:0] mem[2];
  initial begin
    mem[0] = 8'h11;
    mem[1] = 8'h22;
    force mem[1][3:0] = 4'hf;
    #1 $display("1 %h", mem[1]);
    mem[1] = 8'h33;
    #1 $display("2 %h", mem[1]);
    release mem[1][3:0];
    #1 $display("3 %h", mem[1]);
    $finish;
  end
endmodule
