// A range of keys of an associative array is not a slice the frontend accepts,
// so it cannot be a memory_name; the diagnostic comes from the frontend.
module tb;
  int rows [int][4];
  initial $readmemh("words.mem", rows[1:2]);
endmodule
