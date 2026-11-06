// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/reject_wait_call_result.sv
// A property selected through a call result names storage only the call can
// find, so nothing can arm on it: it rejects with its location (SV 9.4.2).
module tb;
    class C;
        int x;
    endclass
    C a[2];
    function automatic C pick();
        return a[0];
    endfunction
    initial begin
        a[0] = new;
        fork
            wait (pick().x == 1);
            #1 a[0].x = 1;
        join
        $finish;
    end
endmodule
