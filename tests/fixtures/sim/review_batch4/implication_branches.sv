// llg-test-fixture: tests/fixtures/sim/review_batch4/implication_branches.sv
module tb;
    bit clk = 0, start = 1;
    int q = 2, failures;
    property captured;
        int v;
        @(posedge clk) ((start, v = 1) or (start, v = 2)) |=> (q == v);
    endproperty
    assert property (captured) else failures++;
    initial begin
        #1 clk = 1;
        #1 begin clk = 0; start = 0; end
        #1 clk = 1;
        #1;
        // The endpoint carrying v=1 must fail even though the v=2 endpoint
        // succeeds. Do not constrain aggregate pass-action accounting here.
        if (failures != 1) $fatal(1, "antecedent endpoints collapsed or lost locals");
        $display("implication branching endpoints ok");
        $finish(0);
    end
endmodule
