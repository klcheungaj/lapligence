// llg-test-fixture: tests/fixtures/sim/review_batch4/implication_locals.sv
module tb;
    bit clk = 0, start = 1;
    int d = 11, q = 0, failures;
    property captured;
        int v;
        @(posedge clk) (start, v = d) |=> (q == v);
    endproperty
    assert property (captured) else failures++;
    initial begin
        #1 clk = 1;
        #1 begin clk = 0; d = 22; q = 11; end
        #1 clk = 1;
        #1 begin clk = 0; start = 0; q = 22; end
        #1 clk = 1;
        #1;
        if (failures != 0) $fatal(1, "overlapping implication snapshots lost");
        $display("implication local transfer ok");
        $finish(0);
    end
endmodule
