// llg-test-fixture: tests/fixtures/sim/u01_coverage/reachable_switch.sv
// A reachable MOS switch: legal, and unsupported by llg by decision (ADV-004),
// so it must stop the run at its own source span.
module tb;
    reg a, g;
    wire y;
    nmos n1(y, a, g);
    initial begin
        a = 0;
        g = 1;
        $finish;
    end
endmodule
