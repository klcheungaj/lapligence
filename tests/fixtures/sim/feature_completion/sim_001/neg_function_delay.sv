// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/neg_function_delay.sv
// Language-illegal neighbour (IEEE 1800-2009 §13.4): a function body cannot
// contain a delay control.
module tb;
    function int f(int x);
        #1;
        return x;
    endfunction
    initial $display("%0d", f(1));
endmodule
