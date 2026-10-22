// SIM-027 A03: a whole associative array whose elements are unpacked arrays
// is not a memory_name, for a dump either (decision S27-D22).
module tb;
  int rows [int][4];
  initial $writememh("words.mem", rows);
endmodule
