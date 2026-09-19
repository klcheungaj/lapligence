// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_19/nba_automatic_local.sv
// G1-19 nba_illegal_lifetime (negative): a nonblocking assignment to automatic
// subroutine storage outlives its activation and must stay rejected.
module tb;
    function automatic void f();
        logic [3:0] v;
        v <= 4'h1;
    endfunction

    initial begin
        f();
        $finish(0);
    end
endmodule
