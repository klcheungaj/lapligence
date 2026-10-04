// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/race_outcomes.sv
// IEEE 1800-2009 §§4.6-4.7, 4.9.4, 10.5 and 10.4.2. Lines prefixed "race"
// come from same-region interprocess races; any listed outcome is permitted
// and the test accepts every one. All other lines are fixed by the standard:
// declaration initializers precede procedures, Inactive (#0) follows every
// time-zero continuous-assignment evaluation, NBAs commit after Inactive,
// $strobe observes the end of the slot and Reactive follows Active.
program p;
    initial begin
        #4 $display("program sees d=%0d", tb.d);
        // Stay live so tb's $finish, not program completion, ends the run.
        #10;
    end
endprogram

module tb;
    int a = 0;
    int b = 0;
    int c = 0;
    int d = 0;
    int x = 9;
    int w;
    assign w = x + 1;
    p p0();

    initial $display("init x=%0d", x);
    initial #0 $display("time0 w=%0d", w);

    // Write-write race: 1 or 2.
    initial #1 a = 1;
    initial #1 a = 2;
    initial #2 $display("race a=%0d", a);

    // Read-write race in one Active region: 0 or 5.
    initial #3 b = 5;
    initial #3 $display("race b=%0d", b);

    // An NBA is never visible to Active or Inactive code of its own slot.
    initial #4 c <= 7;
    initial #4 $display("active c=%0d", c);
    initial #4 #0 $display("inactive c=%0d", c);
    initial #4 $strobe("strobe c=%0d", c);
    initial #4 d = 3;

    initial begin
        #5;
        $display("done b=%0d c=%0d", b, c);
        $finish(0);
    end
endmodule
