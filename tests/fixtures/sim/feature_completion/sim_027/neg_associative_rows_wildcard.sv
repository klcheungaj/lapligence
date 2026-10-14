// The whole of an associative array of unpacked arrays with a wildcard index
// is rejected like the packed-element form (S27-D18).
module tb;
  int rows [*][4];
  initial $readmemh("words.mem", rows);
endmodule
