// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_19/nba_ref_formal.sv
// G1-19 nba_illegal_lifetime (negative): a nonblocking write through a `ref`
// formal aliases caller storage but is not a legal NBA destination. A static
// task reaches the simulator's reference-formal guard (an automatic task is
// already rejected by the frontend).
module tb;
    task static set(ref logic [3:0] t);
        t <= 4'h1;
    endtask

    logic [3:0] q;

    initial begin
        set(q);
        $finish(0);
    end
endmodule
