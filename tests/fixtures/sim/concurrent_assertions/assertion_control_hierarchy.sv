// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/assertion_control_hierarchy.sv
// IEEE 1800-2009 §20.11: hierarchy selectors control assertions in an
// instantiated descendant without changing the assertion's sampled domain.
module leaf(input logic clk, input logic value);
    check: assert property (@(posedge clk) value)
        $display("HIERARCHY_PASS");
endmodule

module tb;
    logic clk;
    logic value;
    leaf dut(.clk(clk), .value(value));

    initial begin
        clk = 1'b0;
        value = 1'b1;
        $assertoff(0, dut);
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            $asserton(0, dut.check);
        end
        #1 clk = 1'b1;
        #1 $finish(0);
    end
endmodule
