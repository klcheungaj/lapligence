// ADV-026 A02: real elements have no memory image.
module tb;
  real rm [0:3];
  initial $sreadmemh(rm, 0, 1, "11 22");
endmodule
