// IEEE 1800-2009 12.6.1: case wildcards apply to tag and payload bits.
module tb;
    typedef union tagged packed {
        void invalid;
        logic [3:0] valid;
    } choice_t;
    typedef union tagged packed {
        void stop;
        choice_t nested;
    } outer_t;
    choice_t value;
    outer_t outer;
    int result, calls;

    function automatic choice_t sample();
        calls++;
        return value;
    endfunction

    initial begin
        calls = 0;
        value = 'x;
        casez (sample()) matches
            tagged invalid: result = 1;
            default: result = 0;
        endcase
        if (result != 0 || calls != 1) $fatal(1, "casez X tag");
        casex (sample()) matches
            tagged invalid: result = 1;
            default: result = 0;
        endcase
        if (result != 1 || calls != 2) $fatal(1, "casex X tag");
        // Exact predicates still require a known active tag.
        if (value matches tagged invalid) $fatal(1, "exact X tag");

        value = 'z;
        casez (sample()) matches
            tagged invalid: result = 1;
            default: result = 0;
        endcase
        if (result != 1 || calls != 3) $fatal(1, "casez Z tag");
        case (value) matches
            tagged invalid: result = 1;
            default: result = 0;
        endcase
        if (result != 0) $fatal(1, "exact Z tag");

        outer = 'x;
        casex (outer) matches
            tagged nested (tagged valid 4'ha): result = 2;
            default: result = 0;
        endcase
        if (result != 2) $fatal(1, "nested casex tag and payload");
        outer = 'z;
        casez (outer) matches
            tagged nested (tagged valid 4'ha): result = 3;
            default: result = 0;
        endcase
        if (result != 3) $fatal(1, "nested casez tag and payload");

        value = tagged valid 4'ha;
        priority case (sample()) matches
            tagged valid .first &&& first == 0: result = 1;
            tagged valid .second &&& second == 4'ha: result = second;
            default: result = 0;
        endcase
        if (result != 10 || calls != 4) $fatal(1, "ordered filter and binding");
        unique casez (value) matches
            tagged invalid: result = 0;
            tagged valid 4'b10??: result = 4;
            default: result = 0;
        endcase
        if (result != 4) $fatal(1, "known tag unique casez");
        if (value.valid !== 4'ha) $fatal(1, "ordinary checked access");
        $display("PASS n09_tagged_case_modes");
        $finish(0);
    end
endmodule
