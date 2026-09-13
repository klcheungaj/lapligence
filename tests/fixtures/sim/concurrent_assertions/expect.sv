// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/expect.sv
// IEEE 1800-2009 §16.18: a procedural expect blocks until its sampled
// property reaches one pass/fail endpoint, then executes its selected action.
module tb;
    logic clk;
    logic ready;

    task automatic wait_ready;
        expect (@(posedge clk) ready)
            $display("EXPECT_PASS");
        else
            $display("EXPECT_FAIL");
    endtask

    initial begin
        clk = 1'b0;
        ready = 1'b1;
        wait_ready();
        $finish(0);
    end

    initial begin
        #2 clk = 1'b1;
    end
endmodule
