// llg-test-fixture: tests/fixtures/sim/regression_81/inactive_disable.sv
// IEEE 1364-2001 section 11: disabling an inactive named block is a no-op.
module tb;
    integer count = 0;

    initial begin : target
        count = 1;
    end

    initial begin
        #1 disable target;
        count = count + 1;
        $display("count=%0d", count);
        $finish(0);
    end
endmodule
