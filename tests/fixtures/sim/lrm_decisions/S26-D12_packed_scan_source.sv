// Decision S26-D12: an integral $sscanf source is read as its characters,
// most significant byte first, ignoring leading zero bytes; a zero byte
// inside the text is white space.
//
// IEEE 1800-2009 21.3.4.3 (SystemVerilog-1800-2009.txt L36858, L36873-36874):
//   "$sscanf reads from the argument str, which may be an expression of
//   integral, unpacked array of byte, or string data type." / "For $sscanf,
//   null characters shall also be considered white space."
//
// llg converts an integral source as a string literal stored in a vector is
// read back (5.9): leading zero bytes are padding and are dropped.
module tb;
  integer c;
  logic [8*8-1:0] src;
  logic [8*3-1:0] mid;
  int a, b;
  initial begin
    src = "7 8";
    c = $sscanf(src, "%d %d", a, b);
    $display("c=%0d a=%0d b=%0d", c, a, b);
    mid = {"5", 8'h00, "6"};
    c = $sscanf(mid, "%d %d", a, b);
    $display("c=%0d a=%0d b=%0d", c, a, b);
    $finish;
  end
endmodule
