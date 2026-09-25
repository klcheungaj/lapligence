// llg-test-fixture: tests/fixtures/sim/sequential_predicates/syn_025_pattern_case.sv
// LRM: IEEE 1800-2009 12.6.1.
// Pattern-case items preserve source order, per-item bindings and filters;
// case/casez/casex qualifiers retain their four-state matching mode.
module tb;
    typedef struct packed {
        logic [3:0] tag;
        logic [3:0] payload;
    } packet_t;

    typedef union tagged packed {
        void invalid;
        logic [7:0] valid;
    } choice_t;

    logic [7:0] value, result;
    packet_t packet;
    choice_t choice;
    int calls;

    function automatic logic [7:0] selected(input logic [7:0] source);
        begin
            selected = 8'h00;
            case (source) matches
                .first &&& first == 8'h00: selected = 8'h10;
                .second &&& second == 8'h5a: selected = second;
                default: selected = 8'hee;
            endcase
        end
    endfunction

    function automatic logic [7:0] sampled();
        begin
            calls = calls + 1;
            sampled = value;
        end
    endfunction

    initial begin
        value = 8'h5a;
        calls = 0;

        result = selected(value);
        if (result !== 8'h5a) $fatal(1, "filtered duplicate patterns");

        case (sampled()) matches
            .captured &&& captured == 8'h5a: result = captured;
            default: result = 8'h00;
        endcase
        if (calls != 1 || result !== 8'h5a)
            $fatal(1, "pattern case selector evaluation");

        choice = tagged valid 8'h5a;
        casez (choice) matches
            tagged invalid: result = 8'h01;
            tagged valid .case_payload &&& case_payload == 8'h5a:
                result = case_payload;
            default: result = 8'h00;
        endcase
        if (result !== 8'h5a)
            $fatal(1, "tagged pattern case item");

        choice = 'x;
        casez (choice) matches
            tagged invalid: result = 8'h01;
            default: result = 8'h00;
        endcase
        if (result !== 8'h00)
            $fatal(1, "casez wildcard ignored tagged X tag");

        casex (choice) matches
            tagged invalid: result = 8'h01;
            default: result = 8'h00;
        endcase
        // SV 12.6.1 includes tag bits in casex wildcard matching.
        if (result !== 8'h01)
            $fatal(1, "casex did not wildcard tagged X tag");

        case (value) matches
            8'h5a: result = 8'h11;
            .*: result = 8'h22;
        endcase
        if (result !== 8'h11) $fatal(1, "constant pattern item");

        packet = '{tag: 4'h3, payload: 4'ha};
        case (packet) matches
            '{tag: 4'h3, payload: .payload} &&& payload == 4'ha:
                result = {4'h3, payload};
            default: result = 8'h00;
        endcase
        if (result !== 8'h3a) $fatal(1, "structure pattern item");

        casez (8'b10xz) matches
            4'b10??: result = 8'h31;
            default: result = 8'h00;
        endcase
        if (result !== 8'h31) $fatal(1, "casez pattern item");

        casex (8'b1x0z) matches
            4'b10?1: result = 8'h32;
            default: result = 8'h00;
        endcase
        if (result !== 8'h32) $fatal(1, "casex pattern item");

        result = 8'h7f;
        case (value) matches
            8'h00: result = 8'h01;
        endcase
        if (result !== 8'h7f) $fatal(1, "pattern case no-match behavior");

        unique case (value) matches
            .duplicate_a: result = duplicate_a;
            .duplicate_b: result = duplicate_b;
        endcase
        if (result !== 8'h5a) $fatal(1, "unique pattern case first match");

        priority case (value) matches
            .priority_value &&& priority_value == 8'h5a: result = priority_value;
            default: result = 8'h00;
        endcase
        unique case (value) matches
            .unique_value &&& unique_value == 8'h5a: result = unique_value;
            default: result = 8'h00;
        endcase
        if (result !== 8'h5a) $fatal(1, "pattern case qualifiers");

        $display("pattern_case=pass result=%h calls=%0d", result, calls);
        $finish(0);
    end
endmodule
