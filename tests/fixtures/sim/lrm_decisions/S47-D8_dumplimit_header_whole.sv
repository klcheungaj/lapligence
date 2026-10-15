// Decision S47-D8: $dumplimit stops value dumping, not the declarations: the
// header is always written whole. The first record that would take the file
// past the limit stops the dump; an open checkpoint gets its `$end` and a
// `$comment dump limit reached $end` follows, even beyond the limit.
//
// IEEE 1800-2009 21.7.1.5 (SystemVerilog-1800-2009.txt L37901-37903): "The
// filesize argument specifies the maximum size of the VCD file in bytes. When
// the size of the VCD file reaches this number of bytes, the dumping stops,
// and a comment is inserted in the VCD file indicating the dump limit was
// reached." The text does not say whether the header counts or where the
// comment goes.
// Compare S47-D8.vcd: the full header, then only the comment.
module tb;
  logic [7:0] a;
  initial begin
    a = 8'h00;
    $dumpfile("S47-D8.vcd");
    $dumplimit(16);
    $dumpvars(0, tb);
    #1 a = 8'h01;
    $strobe("end of step 1: a=%h", a);
    #1 $finish;
  end
endmodule
