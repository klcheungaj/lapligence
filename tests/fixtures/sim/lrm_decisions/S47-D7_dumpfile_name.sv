// Decision S47-D7: the last $dumpfile name executed before the dump starts is
// used, and the file is created only when a $dumpvars starts the dump; a
// $dumpfile after that is ignored with a warning. A name ending in `.fst`
// selects FST; every other name is a VCD file.
//
// IEEE 1800-2009 21.7.1.1 (SystemVerilog-1800-2009.txt L37757-37759): "The
// filename is an expression that is a string literal, string data type, or
// an integral data type containing a character string that names the file to
// be opened. The filename is optional and defaults to the literal string
// dump.vcd if not specified." The example uses `module1.dump`. The text does
// not cover several calls.
// Compare: only S47-D7.dump (a VCD file) is written.
module tb;
  logic v;
  initial begin
    v = 1'b0;
    $dumpfile("S47-D7-first.vcd");
    $dumpfile("S47-D7.dump");
    $dumpvars(0, tb);
    #1 $dumpfile("S47-D7-late.vcd");
    v = 1'b1;
    $strobe("end of step 1: v=%b", v);
    #1 $finish;
  end
endmodule
