// IEEE 1800-2009 11.4.14.3-11.4.14.4: a right-to-left unpack reorders only
// the bits it consumes, so their count must be known before any target is
// written; a selector that reads an earlier target of the same unpack is
// rejected by the owner policy.
module tb;
  logic [7:0] q [0:3];
  logic [3:0] len;
  initial begin
    {<<8{len, q with [0 +: len]}} = 28'h2ABCD00;
    $finish;
  end
endmodule
