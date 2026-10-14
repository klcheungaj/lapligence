// An element of an associative array of unpacked arrays as memory_name is not
// supported; it is rejected when the design is compiled, with its location.
module tb;
  int rows [int][4];
  initial $readmemh("words.mem", rows[3]);
endmodule
