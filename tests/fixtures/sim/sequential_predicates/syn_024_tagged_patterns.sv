// llg-test-fixture: tests/fixtures/sim/sequential_predicates/syn_024_tagged_patterns.sv
// LRM: IEEE 1800-2009 §7.3.2, §11.9, §12.6.
// Tagged patterns check the active member before inspecting or binding the
// selected payload. Nested tags and fixed structure payloads keep their own
// type and source-order checks.
module tb;
    typedef union tagged packed {
        void invalid;
        logic [7:0] valid;
        struct packed {
            logic [3:0] kind;
            logic [3:0] payload;
        } packet;
    } choice_t;

    typedef union tagged packed {
        void stop;
        choice_t nested;
    } outer_t;

    typedef union tagged packed {
        logic [3:0] only;
    } singleton_t;

    choice_t value;
    outer_t outer;
    singleton_t singleton;
    logic [7:0] result;
    int checks;
    int calls;

    function automatic choice_t sample();
        calls = calls + 1;
        sample = value;
    endfunction

    initial begin
        checks = 0;
        calls = 0;
        value = tagged valid 8'h5a;

        if (sample() matches tagged valid .sampled &&& sampled == 8'h5a) begin
            result = sampled;
            checks++;
        end else begin
            $fatal(1, "tagged payload binding or filter");
        end
        if (calls != 1)
            $fatal(1, "tagged source evaluated more than once");

        if (value matches tagged invalid)
            $fatal(1, "wrong active tag matched a void arm");
        if (value matches tagged packet '{kind: 4'hx, payload: 4'hx})
            $fatal(1, "inactive X/Z payload was treated as an active arm");

        value = tagged invalid;
        if (value matches tagged invalid)
            checks++;
        else
            $fatal(1, "void tagged pattern");

        value = tagged packet '{kind: 4'hc, payload: 4'ha};
        if (value matches tagged packet
            '{kind: 4'hc, payload: .packet_payload} &&&
            packet_payload == 4'ha) begin
            result = {4'hc, packet_payload};
            checks++;
        end else begin
            $fatal(1, "tagged structure payload binding");
        end

        outer = tagged nested (tagged packet '{kind: 4'hc, payload: 4'ha});
        if (outer matches tagged nested
            (tagged packet '{kind: 4'hc, payload: .nested_payload}) &&&
            nested_payload == 4'ha) begin
            result = {4'hc, nested_payload};
            checks++;
        end else begin
            $fatal(1, "nested tagged payload pattern");
        end

        result = value matches tagged valid .conditional_payload &&&
            conditional_payload == 8'h5a ? conditional_payload : 8'h00;
        if (result !== 8'h00)
            $fatal(1, "tagged false conditional arm");

        value = tagged valid 8'h5a;
        result = value matches tagged valid .conditional_payload &&&
            conditional_payload == 8'h5a ? conditional_payload : 8'h00;
        if (result !== 8'h5a)
            $fatal(1, "tagged true conditional arm");

        singleton = tagged only 4'hc;
        if (singleton matches tagged only .single_payload &&&
            single_payload == 4'hc)
            checks++;
        else
            $fatal(1, "single-member tagged pattern");

        if (checks != 5)
            $fatal(1, "tagged pattern count %0d", checks);
        $display("tagged_patterns=pass checks=%0d calls=%0d", checks, calls);
        $finish(0);
    end
endmodule
