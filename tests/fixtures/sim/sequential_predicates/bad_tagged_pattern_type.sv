// llg-test-fixture: tests/fixtures/sim/sequential_predicates/bad_tagged_pattern_type.sv
// LRM: IEEE 1800-2009 §7.3.2, §12.6.
// Single-fault control: a tagged pattern is applied to an integral source.
module tb;
    logic [1:0] value;

    initial begin
        value = 2'b01;
        if (value matches tagged valid)
            $display("unreachable");
        $finish(0);
    end
endmodule
