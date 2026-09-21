// llg-test-fixture: tests/fixtures/sim/rtl_completion/syn_006_generate_continuous_conflict.sv
// LRM: IEEE 1800-2009 §10.3; IEEE 1800-2005 §11.5.
module generated_conflict (
    output logic value
);
    assign value = 1'b0;
    assign value = 1'b1;
endmodule

module tb;
    logic value;
    generated_conflict u_conflict(.value(value));
endmodule
