// Decision S26-D17 (llg limit): the data arguments of $sreadmemb/$sreadmemh are
// string literals; llg rejects a string variable or other expression at
// compile time, with its source location, and runs nothing.
//
// IEEE 1800-2009 D.14 (SystemVerilog-1800-2009.txt L70157-70158): "The
// $sreadmemh and $sreadmemb system tasks take memory data values and
// addresses as string literal arguments." Annex D is informative (D.1,
// L69791: "for informative purposes only and are not part of this standard"),
// and the syntax in L70152-70153 names the arguments `string`. A simulator
// that also accepts a string variable prints "m 11 22"; llg prints nothing and
// exits with status 1.
module tb;
  logic [7:0] m [0:1];
  string image = "11 22";
  initial begin
    $sreadmemh(m, 0, 1, image);
    $display("m %h %h", m[0], m[1]);
    $finish;
  end
endmodule
