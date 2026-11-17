// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_19/nba_ordered_updates.sv
// G1-19 nba_ordered_updates: source-ordered NBAs to one variable commit in
// issue order, including overlapping masked part-select updates. The
// overridden updates use a run-time-indexed target or a function with a
// side effect, so lowering cannot drop them and the NBA region commits
// every one.
module tb;
    logic [7:0] x;
    logic [7:0] y;
    logic [3:0] a;
    logic [3:0] b;
    int lo;
    int calls;

    function automatic logic [7:0] counted(logic [7:0] value);
        calls++;
        return value;
    endfunction

    initial begin
        x = 8'h00;
        y = 8'h00;
        a = 4'hc;
        b = 4'hd;
        lo = 0;
        x[lo+:4] <= a;    // low nibble <- c
        x[7:4] <= b;      // high nibble <- d
        x[3:0] <= 4'h5;   // later ordered low-nibble update wins
        y <= counted(8'h1);
        y <= counted(8'h2);
        y <= 8'h3;
        #1 $display("x=%h y=%h", x, y);
        $finish(0);
    end
endmodule
