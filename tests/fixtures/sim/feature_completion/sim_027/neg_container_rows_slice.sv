// SIM-027 A03: a slice of the elements of a queue whose elements are unpacked
// arrays is not a memory_name (decision S27-D22).
module tb;
  int rows [$][4];
  initial begin
    rows.push_back('{1, 2, 3, 4});
    $readmemh("words.mem", rows[0:0]);
  end
endmodule
