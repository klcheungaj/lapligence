// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/assertion_control.sv
// IEEE 1800-2009 §20.11: assertion control tasks select named concurrent
// assertions by hierarchy. Use the edition-appropriate $asserton task.
module tb;
    logic clk;
    logic value;
    logic delayed_value;
    logic level;

    off_case: assert property (@(posedge clk) value)
        $display("OFF_REENABLED");
    on_case: assert property (@(posedge clk) value)
        $display("ON_PASS");
    kill_case: assert property (@(posedge clk) value |=> delayed_value)
        $display("KILL_BAD");

    initial begin
        clk = 1'b0;
        value = 1'b1;
        delayed_value = 1'b0;
        level = 1'b0;
        $assertoff(level, off_case);
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            $assertkill(0, kill_case);
            $assertoff(0, kill_case);
            $asserton(0, off_case);
        end
        #1 clk = 1'b1;
        $asserton();
        #1 $finish(0);
    end
endmodule
