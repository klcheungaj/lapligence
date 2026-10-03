// V2001 Annex A; SV2009 11.13: `let` is a SystemVerilog-2009 declaration and
// is not Verilog-2001 syntax.
module tb;
  let inc(x) = x + 1;
  initial begin
    $display("%0d", inc(1));
    $finish;
  end
endmodule
