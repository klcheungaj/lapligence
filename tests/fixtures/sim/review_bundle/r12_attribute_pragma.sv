// llg-test-fixture: tests/fixtures/sim/review_bundle/r12_attribute_pragma.sv
// IEEE 1800-2009 §§5.12, 22.11: attribute instances and pragma expressions
// are accepted by the frontend; the annotation and diagnostic stack do not
// change this fixture's simulated value.
`pragma diagnostic push
(* syn038_note = "metadata", syn038_flag *)
module tb;
    logic value;

    initial begin
        value = 1'b1;
        $display("attribute_pragma=%b", value);
        $finish(0);
    end
endmodule
`pragma diagnostic pop
