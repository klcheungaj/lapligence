// llg-test-fixture: tests/fixtures/sim/syn024_tagged_patterns/case_modes.sv
// IEEE 1800-2009 12.6.1: casez/casex matching applies to tag and payload
// bits. Ordinary member access still checks the exact active tag (7.3.2).
module tb;
    typedef union tagged packed {
        void invalid;
        logic [3:0] valid;
    } choice_t;
    typedef union tagged packed {
        void stop;
        choice_t nested;
    } outer_t;
    choice_t choice;
    outer_t outer;
    int result, checks, calls;

    function automatic choice_t sample();
        calls++;
        return choice;
    endfunction

    initial begin
        checks = 0;
        calls = 0;
        choice = 'x;
        casez (sample()) matches
            tagged invalid: result = 1;
            default: result = 0;
        endcase
        if (result != 0) $fatal(1, "casez X tag");
        checks++;
        casex (sample()) matches
            tagged invalid: result = 1;
            default: result = 0;
        endcase
        if (result != 1) $fatal(1, "casex X tag");
        checks++;
        if (choice matches tagged invalid) $fatal(1, "exact if X tag");
        checks++;

        choice = 'z;
        casez (sample()) matches
            tagged invalid: result = 1;
            default: result = 0;
        endcase
        if (result != 1) $fatal(1, "casez Z tag");
        checks++;
        case (sample()) matches
            tagged invalid: result = 1;
            default: result = 0;
        endcase
        if (result != 0) $fatal(1, "exact case Z tag");
        checks++;

        outer = 'x;
        casex (outer) matches
            tagged nested (tagged valid 4'ha): result = 2;
            default: result = 0;
        endcase
        if (result != 2) $fatal(1, "nested casex");
        checks++;
        outer = 'z;
        casez (outer) matches
            tagged nested (tagged valid 4'ha): result = 3;
            default: result = 0;
        endcase
        if (result != 3) $fatal(1, "nested casez");
        checks++;

        choice = tagged valid 4'ha;
        if (choice.valid !== 4'ha) $fatal(1, "ordinary exact member access");
        checks++;
        if (checks != 8 || calls != 4) $fatal(1, "count");
        $display("tagged_case_modes=pass checks=%0d calls=%0d", checks, calls);
        $finish(0);
    end
endmodule
