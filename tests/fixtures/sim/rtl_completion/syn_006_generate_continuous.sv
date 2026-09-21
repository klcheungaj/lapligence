// llg-test-fixture: tests/fixtures/sim/rtl_completion/syn_006_generate_continuous.sv
// LRM: IEEE 1800-2009 §§10.3, 27.5; IEEE 1364-2001 §12.4.
module generated_driver #(parameter bit SELECT = 1'b1) (
    output logic generated
);
    generate
        if (SELECT) begin : selected
            assign generated = 1'b1;
        end else begin : alternate
            assign generated = 1'b0;
        end
    endgenerate
endmodule

module tb;
    logic selected_value;
    logic alternate_value;

    generated_driver #(.SELECT(1'b1)) selected_instance(.generated(selected_value));
    generated_driver #(.SELECT(1'b0)) alternate_instance(.generated(alternate_value));

    initial begin
        #1;
        $display("generated=%b,%b", selected_value, alternate_value);
        $finish(0);
    end
endmodule
