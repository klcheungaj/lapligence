// llg-test-fixture: tests/fixtures/sim/partial_features/system_invalid_arity.sv
// IEEE 1800-2009 §20.18: `$system` accepts at most one optional command.
module tb;
    initial begin
        $system("echo llg_system_invalid", "extra");
        $finish(0);
    end
endmodule
