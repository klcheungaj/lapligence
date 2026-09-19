// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_06/effects_impure_helper.sv
// IEEE 1800-2009 13.4.1: a function that writes visible state has an effect
// summary outside the read-only evaluator set. It must be rejected with a
// precise diagnostic rather than silently inlined as a pure callback.
module tb;
    int visible = 0;

    function automatic bit bump();
        visible = visible + 1;   // externally visible write
        bump = 1;
    endfunction

    initial @(bump()) $finish(0);
endmodule
