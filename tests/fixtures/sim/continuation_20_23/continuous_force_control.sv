// llg-test-fixture: SYN-006 force/release are overrides; disjoint bits are independent.
module tb;
    logic [64:0] source = 65'd2;
    logic [64:0] destination;
    logic [64:0] disjoint;
    assign destination = source;
    assign disjoint[64:1] = source[64:1];
    initial begin
        disjoint[0] = 1'b1;
        #1;
        if (destination !== 65'd2 || disjoint !== 65'd3) $fatal(1, "initial independent driver");
        force destination = 65'd9;
        #1;
        if (destination !== 65'd9) $fatal(1, "force override");
        release destination;
        source = 65'd4;
        #1;
        if (destination !== 65'd4 || disjoint !== 65'd5) $fatal(1, "released driver");
        $display("CONTINUOUS_FORCE_CONTROL_PASS");
        $finish(0);
    end
endmodule
