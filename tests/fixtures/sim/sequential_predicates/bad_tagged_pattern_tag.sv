// llg-test-fixture: tests/fixtures/sim/sequential_predicates/bad_tagged_pattern_tag.sv
// LRM: IEEE 1800-2009 §7.3.2, §12.6.
// Single-fault control: the tagged pattern names a member absent from the
// matched union type.
module tb;
    typedef union tagged packed {
        void invalid;
        logic [1:0] valid;
    } choice_t;
    choice_t value;

    initial begin
        value = tagged valid 2'b01;
        if (value matches tagged missing)
            $display("unreachable");
        $finish(0);
    end
endmodule
