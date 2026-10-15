// llg-test-fixture: SIM-042 A03 absent registration. $absent is declared to
// the compiler but errors.c never registers it, so the model must stop at
// startup with a VPI error instead of running the call as a no-op.
module tb;
    initial begin
        $absent;
        $display("unreachable");
    end
endmodule
