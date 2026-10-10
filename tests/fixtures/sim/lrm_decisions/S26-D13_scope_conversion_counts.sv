// Decision S26-D13: %m assigns the calling scope's hierarchical name and
// counts as an assigned item.
//
// IEEE 1800-2009 21.3.4.3 (SystemVerilog-1800-2009.txt L36995-36996,
// L37023):
//   "m Returns the current hierarchical path as a string. Does not read data
//   from the input file or str argument." / "The number of successfully
//   matched and assigned input items is returned in code"
//
// %m assigns a value, so llg counts it, even when it is the only directive.
module tb;
  integer c;
  string s;
  initial begin
    c = $sscanf("x", "%m", s);
    $display("c=%0d s=%s", c, s);
    $finish;
  end
endmodule
