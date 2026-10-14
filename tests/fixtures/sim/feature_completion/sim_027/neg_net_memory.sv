// A load assigns words like procedural assignments; a net array is not a
// variable memory (IEEE 1800-2009 7.4.4, 21.4).
module tb;
  wire [7:0] mem [0:3];
  initial $readmemh("in.hex", mem);
endmodule
