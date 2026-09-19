// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_19/nba_issue_capture.sv
// G1-19 nba_issue_capture: changing the RHS or an index after enqueue must not
// change the scheduled value or destination. IEEE 1800-2009 10.4.2.
module tb;
    logic [7:0] x;
    logic [3:0] a;
    logic [3:0] mem [0:1];
    int i;

    initial begin
        x = 8'h00;
        mem[0] = 4'h0;
        mem[1] = 4'h0;
        a = 4'h5;
        x[3:0] <= a;      // captures 5
        a = 4'h9;         // must not affect the queued low nibble
        i = 0;
        mem[i] <= a;      // captures destination 0, value 9
        i = 1;
        mem[i] <= 4'hb;   // captures destination 1, value b
        #1 $display("x=%h mem0=%h mem1=%h", x, mem[0], mem[1]);
        $finish(0);
    end
endmodule
