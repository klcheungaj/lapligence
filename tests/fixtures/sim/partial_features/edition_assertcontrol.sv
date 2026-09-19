// llg-test-fixture: tests/fixtures/sim/partial_features/edition_assertcontrol.sv
// `$assertcontrol` is a SystemVerilog assertion-control subroutine; it is not
// part of IEEE 1364-2001 Verilog.
module tb;
  initial begin
    $assertcontrol(1);
    $display("done");
    $finish;
  end
endmodule
