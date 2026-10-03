// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_06/effects_impure_helper.sv
// IEEE 1800-2009 13.4.1, 9.4.2: a function that writes visible state is a
// legal event helper, but not a read-only callback. The waiting process
// evaluates it; a constant result never triggers, and the write stays visible.
module tb;
    int visible = 0;

    function automatic bit bump();
        visible = visible + 1;   // externally visible write
        bump = 1;
    endfunction

    initial begin
        fork
            @(bump()) $display("fired");
            #1 $display("unchanged evaluated=%0d", visible > 0);
        join_any
        $finish(0);
    end
endmodule
