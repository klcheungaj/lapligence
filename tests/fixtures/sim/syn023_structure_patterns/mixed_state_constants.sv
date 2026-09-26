// llg-test-fixture: tests/fixtures/sim/syn023_structure_patterns/mixed_state_constants.sv
// IEEE 1800-2009 7.2.1, 12.6: a bit member is converted on read, while
// a logic member retains X/Z for exact constant-pattern matching in if.
module tb;
    typedef struct packed {
        logic [3:0] four;
        bit [3:0] two;
    } mixed_t;
    mixed_t value;
    int checks;
    initial begin
        value = 'x;
        checks = 0;
        if (value.two !== 4'h0) $fatal(1, "two-state member read");
        if (value matches '{two: 4'h0}) checks++;
        else $fatal(1, "two-state member constant zero");
        if (value matches '{four: 4'hx}) checks++;
        else $fatal(1, "four-state member X constant");
        if (value matches '{two: 4'hx}) $fatal(1, "two-state member X constant");
        else checks++;
        if (value matches '{four: 4'h0}) $fatal(1, "four-state member zero constant");
        else checks++;
        $display("mixed_state_patterns=pass checks=%0d", checks);
        $finish(0);
    end
endmodule
