// llg-test-fixture: tests/fixtures/sim/syn024_tagged_patterns/edition_boundary.sv
// IEEE 1364-2001 has neither tagged unions nor pattern predicates.
module tb;
    typedef union tagged packed {
        void invalid;
        logic valid;
    } choice_t;
    choice_t choice;
    initial begin
        choice = tagged valid 1'b1;
        if (choice matches tagged valid 1'b1)
            $display("tagged_edition=pass");
        else $fatal(1, "tagged pattern in 2009");
        $finish(0);
    end
endmodule
