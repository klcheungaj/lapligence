// An associative memory needs an index of integral type (IEEE 1800-2009
// 21.4.1, 21.5.3). A wildcard index has no index type to cast file addresses
// to, so llg rejects it with a located diagnostic (decision S27-D18).
module tb;
  logic [7:0] mem [*];
  initial $readmemh("in.hex", mem);
endmodule
