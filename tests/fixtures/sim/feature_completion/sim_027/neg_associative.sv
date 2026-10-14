// Associative memories are legal (IEEE 1800-2009 21.4.1) but llg does not
// load or dump them yet; the call is rejected before a model is built.
module tb;
  logic [7:0] mem [int];
  initial $readmemh("in.hex", mem);
endmodule
