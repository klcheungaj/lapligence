// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/runtime_binding.sv
// RTL-018, T1-D6 (SV 2009 §33.7 L54029): "The format specifier %l or %L shall
// print out the library.cell binding information for the module instance
// containing the display (or other textual output) command." A format built
// at run time prints the same text as a literal one.
module tb;
  string format = "%l";
  rtl018_runtime_bound u();
  initial #1 begin
    $display("%s", $sformatf(format));
    $finish;
  end
endmodule
