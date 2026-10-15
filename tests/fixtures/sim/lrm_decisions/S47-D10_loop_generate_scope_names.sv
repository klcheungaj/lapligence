// Decision S47-D10: a loop generate block instance is the scope `g[i]` of the
// enclosing scope in waveform names, with no level for the loop construct:
// the dump declares tb.g[0].l.v, as %m prints tb.g[0].l.
//
// IEEE 1800-2009 27.4 (SystemVerilog-1800-2009.txt L46317-46320): "For each
// block instance created by the generate loop, the generate block identifier
// for the loop is indexed by adding the “[genvar value]” to the end of the
// generate block identifier. These names can be used in hierarchical path
// names (see 23.6)." Example 5 (L46327): "// scope B1[i]".
// Compare S47-D10.vcd: `$scope module g[0]` (written `g$5B0$5D`) directly in
// `tb`.
module leaf #(parameter int D = 1);
  logic v;
  initial begin
    v = 1'b1;
    #D $display("%m v=%b", v);
  end
endmodule

module tb;
  for (genvar i = 0; i < 2; i++) begin : g
    leaf #(.D(i + 1)) l();
  end
  initial begin
    $dumpfile("S47-D10.vcd");
    $dumpvars(0, tb);
    #3 $finish;
  end
endmodule
