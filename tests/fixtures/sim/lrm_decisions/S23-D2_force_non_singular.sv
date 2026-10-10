// Decision S23-D2: an unpacked structure (or unpacked array) variable is not
// a legal force or release target.
//
// IEEE 1800-2009 10.6.2 (SystemVerilog-1800-2009.txt L13373-13375):
//   "The left-hand side of the assignment can be a reference to a singular
//   variable, a net, a constant bit-select of a vector net, a constant
//   part-select of a vector net, or a concatenation of these."
// IEEE 1800-2009 6.4 (L4606-4607):
//   "A singular type shall be any data type except an unpacked structure,
//   unpacked union, or unpacked array (see 7.4 on arrays)."
//
// llg rejects the design at compile time with the target name and location.
// Expected result: the design is rejected; it prints nothing (the .out file
// is empty). A simulator that accepts it would print "a=5 b=6".
module tb;
  typedef struct {
    logic [3:0] a;
    logic [3:0] b;
  } us_t;
  us_t us;
  initial begin
    force us = '{a: 4'h5, b: 4'h6};
    #1 $display("a=%h b=%h", us.a, us.b);
    $finish;
  end
endmodule
