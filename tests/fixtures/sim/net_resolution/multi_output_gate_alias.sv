// llg-test-fixture: tests/fixtures/sim/net_resolution/multi_output_gate_alias.sv
// LRM: IEEE 1800-2009 10.11.
module tb;
    logic input_value;
    wire a, b;
    alias a = b;
    buf (a, b, input_value);

    initial begin
        input_value = 1'b1;
        #1 $display("CHECK: multi_gate=%b%b", a, b);
        input_value = 1'b0;
        #1 $display("CHECK: multi_gate=%b%b", a, b);
        $finish(0);
    end
endmodule
