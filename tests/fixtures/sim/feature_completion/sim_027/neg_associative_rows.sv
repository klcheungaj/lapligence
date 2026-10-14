// An element of an associative array of unpacked arrays with a string index
// has no index type to cast file addresses to (IEEE 1800-2009 21.4.1); it is
// rejected when the design is compiled, with its location.
module tb;
  int rows [string][4];
  initial $readmemh("words.mem", rows["x"]);
endmodule
