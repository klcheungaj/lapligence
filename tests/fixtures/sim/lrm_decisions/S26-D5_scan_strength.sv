// Decision S26-D5: %v reads one three-character strength and assigns its
// four-state value.
//
// IEEE 1800-2009 21.3.4.3 Table 21-8 (SystemVerilog-1800-2009.txt
// L36927-36931):
//   "v Matches a net signal strength, consisting of a three-character
//   sequence as specified in 21.2.1.5. ... (if assigned to integral
//   variables, the values are converted to the 4 value equivalent)."
//
// llg accepts the 21.2.1.5 forms: a strength mnemonic (Su St Pu La We Me Sm)
// or two strength digits followed by 0, 1, X, Z, L or H, and HiZ. The value
// character gives the result (L and H read as X, HiZ as Z), placed in bit 0
// with the other bits cleared.
module tb;
  integer c;
  logic [4:0] v;
  logic [3:0] w;
  initial begin
    c = $sscanf("St1 We0 HiZ Pu1 StX", "%v %v %v %v %v", v[0], v[1], v[2], v[3], v[4]);
    $display("c=%0d v=%b", c, v);
    w = 4'b1111;
    c = $sscanf("Su0", "%v", w);
    $display("c=%0d w=%b", c, w);
    $finish;
  end
endmodule
