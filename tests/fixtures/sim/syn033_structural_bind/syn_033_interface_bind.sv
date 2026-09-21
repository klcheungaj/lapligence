// llg-test-fixture: tests/fixtures/sim/syn033_structural_bind/syn_033_interface_bind.sv
// IEEE 1800-2009 §23.11: an interface-type bind may target an interface
// instance, whose finite combinational body remains in the owned hierarchy.
interface syn033_if (
  input logic a
);
  logic bound;
endinterface

interface syn033_if_observer (
  input logic a,
  output logic observed
);
  assign observed = ~a;
endinterface

module tb;
  logic a;
  syn033_if bus(.a(a));

  initial begin
    a = 1'b0;
    #1;
    $display("interface_bind=%b", bus.bound);
    a = 1'b1;
    #1;
    $display("interface_bind=%b", bus.bound);
    $finish;
  end
endmodule

bind syn033_if syn033_if_observer by_interface(
  .a(a),
  .observed(bound)
);
