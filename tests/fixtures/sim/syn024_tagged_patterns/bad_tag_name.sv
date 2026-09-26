// llg-test-fixture: tests/fixtures/sim/syn024_tagged_patterns/bad_tag_name.sv
// IEEE 1800-2009 7.3.2, 12.6: the tag must name a member of the source union.
module tb;
    typedef union tagged packed { void invalid; logic valid; } choice_t;
    choice_t choice;
    initial begin
        choice = tagged valid 1'b1;
        if (choice matches tagged missing) $display("unreachable");
    end
endmodule
