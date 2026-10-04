// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_19/nba_ref_formal.sv
// G1-19 nba_illegal_lifetime (negative): a nonblocking write through a `ref`
// formal aliases caller storage but is not a legal NBA destination. The
// frontend rejects both forms: an automatic task for the NBA, and this static
// task because IEEE 1800-2009 13.5.2 forbids ref formals in static
// subroutines.
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
