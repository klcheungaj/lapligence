// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/restored.sv
// Values changed and restored within a time step (decision S47-D14), the
// overridden nonblocking updates lowering merges (FIN-001 O2), and the
// per-delta dump (`--wave-deltas`, S47-D15). With `KEEP` defined, the
// overridden updates take their value from a function call, which lowering
// never merges, so the NBA region commits each of them; the default dump is
// the same file either way.
`timescale 1ns/1ns
`ifndef FILE
`define FILE "restored.vcd"
`endif
module tb;
    logic clk;
    logic [3:0] q = 4'd0;
    logic [3:0] r = 4'd0;
    logic [3:0] v;
    logic g;
    real level;
    event tick;

`ifdef KEEP
    function automatic logic [3:0] kept(logic [3:0] value);
        return value;
    endfunction
`define V(x) kept(x)
`else
`define V(x) (x)
`endif

    // Step 1: r is written between two updates of q; step 2: q returns to
    // its value within the batch.
    always @(posedge clk or negedge clk) begin
        if (clk) begin
            q <= `V(4'd1);
            r <= r + 4'd5;
            q <= q + 4'd2;
        end else begin
            q <= `V(4'd3);
            q <= q;
        end
    end

    initial begin
        clk = 1'b0;
        v = 4'bx01z;
        g = 1'b0;
        level = 0.5;
        $dumpfile(`FILE);
        $dumpvars(0, tb);
        #1 clk = 1'b1;
        $strobe("1: q=%0d r=%0d", q, r);
        #1 clk = 1'b0;
        $strobe("2: q=%0d r=%0d", q, r);
        // Step 3: every change is undone within one process turn.
        #1 v = 4'b0000;
        v = 4'bx01z;
        level = 1.5;
        level = 0.5;
        g = 1'b1;
        g = 1'b0;
        $strobe("3: v=%b level=%0.1f g=%b", v, level, g);
        // Step 4: undone in a later delta of the step.
        #1 v = 4'b0110;
        #0 v = 4'bx01z;
        $strobe("4: v=%b", v);
        // Step 5: two triggers are one change of the event.
        #1 -> tick;
        -> tick;
        // Step 6: a change; step 7: a $dumpall checkpoint, then a change
        // undone after it.
        #1 v = 4'b1111;
        #1 $dumpall;
        v = 4'b0001;
        v = 4'b1111;
        $strobe("7: v=%b", v);
        #1 $finish(0);
    end
endmodule
