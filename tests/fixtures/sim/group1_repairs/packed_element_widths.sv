// R05: selects count multi-bit elements, including residual packed dimensions.
module tb;
  logic [1:0][3:0][7:0] mem [0:0];
  logic [1:0][0:3][7:0] ascending [0:0];
  logic [1:0][1:0][3:0][7:0] deep [0:0];
  logic [2:1][7:0] selected;
  int lane, index;
  initial begin
    mem[0] = 64'h1122334455667788;
    if ($bits(mem[0][1][2:1]) != 16 || mem[0][1][2:1] !== 16'h2233)
      $fatal(1, "ordinary multi-byte read");
    selected = mem[0][1][2:1];
    if (selected[1] !== 8'h33) $fatal(1, "selected type bounds");
    mem[0][1][2:1] = 16'hbeef;
    if (mem[0] !== 64'h11beef4455667788) $fatal(1, "ordinary multi-byte write");
    lane = 1; index = 1;
    if ($bits(mem[0][lane][index +: 2]) != 16)
      $fatal(1, "indexed element stride width");
    mem[0][lane][index +: 2] = 16'hcafe;
    if (mem[0] !== 64'h11cafe4455667788) $fatal(1, "runtime multi-byte write");
    index = 2;
    if (mem[0][lane][index -: 2] !== 16'hcafe) $fatal(1, "runtime multi-byte read");
    ascending[0] = 64'h1122334455667788;
    index = 1;
    ascending[0][1][index +: 2] = 16'hbeef;
    if (ascending[0] !== 64'h11beef4455667788) $fatal(1, "ascending byte stride");
    index = 2;
    if (ascending[0][1][index -: 2] !== 16'hbeef) $fatal(1, "ascending byte read");
    // Partially outside a 4-byte lane: high byte of the result is X, not a
    // byte of the adjacent enclosing lane. The valid result byte is preserved.
    mem[0] = 64'haabbccdd11223344;
    mem[0][0][3 +: 2] = 16'hface;
    if (mem[0] !== 64'haabbccddce223344) $fatal(1, "byte lane clipping");
    if (mem[0][0][3 +: 2] !== 16'hxxce) $fatal(1, "byte lane read padding");
    deep[0] = 128'h0000000000000000aabbccddeeff0011;
    deep[0][1][0][2:1] = 16'h1234;
    if ($bits(deep[0][1][0][2:1]) != 16 ||
        deep[0] !== 128'h0000000000123400aabbccddeeff0011)
      $fatal(1, "four packed dimensions");
    $display("packed element widths passed");
    $finish(0);
  end
endmodule
