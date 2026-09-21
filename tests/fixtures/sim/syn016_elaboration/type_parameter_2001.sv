// llg-test-fixture: tests/fixtures/sim/syn016_elaboration/type_parameter_2001.sv
// IEEE 1800-2009 §6.20.3 is unavailable under the strict Verilog-2001 policy.
module tb #(parameter type T = integer);
  initial $finish;
endmodule
