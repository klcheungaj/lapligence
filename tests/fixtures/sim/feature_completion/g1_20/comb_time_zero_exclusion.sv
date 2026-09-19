// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_20/comb_time_zero_exclusion.sv
// G1-20 comb_time_zero_and_write_exclusion: always_comb runs at time zero and
// does not make itself sensitive to the expressions it writes.
module tb;
    logic a;
    logic [3:0] q;
    logic [3:0] o;

    always_comb begin
        q = {3'b0, a};
        o = q;
    end

    initial begin
        a = 1'b0;
        #1 $display("t1 q=%h o=%h", q, o);
        a = 1'b1;
        #1 $display("t2 q=%h o=%h", q, o);
        $finish(0);
    end
endmodule
