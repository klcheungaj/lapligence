// llg-test-fixture: tests/fixtures/sim/syn018_module_declarations/extern_specializations.sv
// IEEE 1800-2009 §23.5: the matching extern header and body specialize
// parameter-dependent ports for two instance widths across source files.
module tb;
  logic [3:0] a4, y4;
  logic [4:0] a5, y5;
  syn018_extern_child #(.W(4)) e4(.a(a4), .y(y4));
  syn018_extern_child #(.W(5)) e5(.a(a5), .y(y5));

  initial begin
    a4 = 4'd3;
    a5 = 5'd3;
    #1 $display("extern_specialized=%0d/%0d widths=%0d/%0d", y4, y5, $bits(y4), $bits(y5));
    $finish;
  end
endmodule
