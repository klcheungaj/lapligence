// SIM-027 A03: a whole dynamic array whose elements are unpacked arrays is
// not a memory_name (decision S27-D22); one element (`rows[k]`) is.
module tb;
  int rows [][4];
  initial begin
    rows = new[2];
    $readmemh("words.mem", rows);
  end
endmodule
