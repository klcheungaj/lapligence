// ADV-026 A02: a wildcard-index associative array has no index type to cast
// the addresses to (decision S27-D18 applies to the string-sourced task too).
module tb;
  int aa [*];
  initial $sreadmemh(aa, 0, 1, "11 22");
endmodule
