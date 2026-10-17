// ADV-026 A02: string elements have no memory image.
module tb;
  string sm [0:3];
  initial $sreadmemh(sm, 0, 1, "11 22");
endmodule
