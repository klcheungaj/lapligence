// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/region_reentry.sv
// IEEE 1800-2009 §§4.4.1-4.5 (reference algorithm), 9.4.1 (#0), 10.4.2 and
// 24.3.1: Active -> Inactive -> NBA -> Active re-entry in the design set, and
// Reactive -> Re-Inactive -> Re-NBA -> Active re-entry from a program, with
// the reactive set drained completely before the design set resumes. Every
// printed line is ordered by the reference algorithm alone; no two lines come
// from processes whose relative order the standard leaves open.
program p;
    initial begin
        #10;
        // Reactive: design work enabled here (the always @(b) below) must
        // wait until Reactive, Re-Inactive and Re-NBA are all empty.
        tb.b = 1;
        tb.b <= 2;
        $display("R reactive b=%0d", tb.b);
        #0 $display("R re-inactive b=%0d", tb.b);
        // Re-NBA has not committed yet: a second #0 still sees 1.
        #0 $display("R re-inactive2 b=%0d", tb.b);
        #5;
        // The last program to finish ends the simulation (§24.7).
        $display("R t=%0d b=%0d c=%0d", $time, tb.b, tb.c);
    end
endprogram

module tb;
    int a = 0;
    int b = 0;
    int c = 0;
    p p0();

    initial begin
        #1;
        a = 1;
        a <= 2;
        $display("A active a=%0d", a);
        #0 $display("A inactive a=%0d", a);
        #0 $display("A inactive2 a=%0d", a);
    end

    // Wakes once from the blocking write and once from the NBA commit.
    always @(a) $display("A woke a=%0d", a);

    // The program's blocking write and its Re-NBA both land before this
    // process runs, so it observes only the committed value.
    always @(b) begin
        $display("A from reactive b=%0d", b);
        c <= b + 10;
        #0 $display("A inactive after reactive c=%0d", c);
    end

    initial begin
        #12;
        $display("A t=%0d a=%0d b=%0d c=%0d", $time, a, b, c);
    end
endmodule
