// llg-test-fixture: tests/fixtures/sim/syn025_pattern_cases/runtime_modes.sv
// IEEE 1800-2009 12.6.1 and 12.5.1: casez wildcards Z/? on either side;
// casex wildcards X/Z/? on either side; case matches exactly.
module tb;
    typedef struct packed {
        logic [3:0] tag;
        logic [3:0] payload;
    } packet_t;

    logic [3:0] selector;
    packet_t packet;
    int result, checks, calls;

    function automatic logic [3:0] sampled();
        calls++;
        return selector;
    endfunction

    function automatic packet_t sampled_packet();
        calls++;
        return packet;
    endfunction

    initial begin
        checks = 0;
        calls = 0;
        if (!$value$plusargs("syn025_selector=%b", selector)) selector = 4'b10z1;

        case (selector) matches
            4'b1001: result = 1;
            default: result = 0;
        endcase
        if (result !== 0) $fatal(1, "exact selector Z variable");
        checks++;
        casez (selector) matches
            4'b1001: result = 1;
            default: result = 0;
        endcase
        if (result !== 1) $fatal(1, "casez selector Z variable");
        checks++;
        casex (selector) matches
            4'b1001: result = 1;
            default: result = 0;
        endcase
        if (result !== 1) $fatal(1, "casex selector Z variable");
        checks++;

        case (sampled()) matches
            4'b1001: result = 1;
            default: result = 0;
        endcase
        if (result !== 0 || calls != 1) $fatal(1, "exact selector Z call");
        checks++;
        casez (sampled()) matches
            4'b1001: result = 1;
            default: result = 0;
        endcase
        if (result !== 1 || calls != 2) $fatal(1, "casez selector Z call");
        checks++;
        casex (sampled()) matches
            4'b1001: result = 1;
            default: result = 0;
        endcase
        if (result !== 1 || calls != 3) $fatal(1, "casex selector Z call");
        checks++;

        selector[1] = 1'bx;
        case (selector) matches
            4'b1001: result = 1;
            default: result = 0;
        endcase
        if (result !== 0) $fatal(1, "exact selector X variable");
        checks++;
        casez (selector) matches
            4'b1001: result = 1;
            default: result = 0;
        endcase
        if (result !== 0) $fatal(1, "casez selector X variable");
        checks++;
        casex (selector) matches
            4'b1001: result = 1;
            default: result = 0;
        endcase
        if (result !== 1) $fatal(1, "casex selector X variable");
        checks++;

        case (sampled()) matches
            4'b1001: result = 1;
            default: result = 0;
        endcase
        if (result !== 0 || calls != 4) $fatal(1, "exact selector X call");
        checks++;
        casez (sampled()) matches
            4'b1001: result = 1;
            default: result = 0;
        endcase
        if (result !== 0 || calls != 5) $fatal(1, "casez selector X call");
        checks++;
        casex (sampled()) matches
            4'b1001: result = 1;
            default: result = 0;
        endcase
        if (result !== 1 || calls != 6) $fatal(1, "casex selector X call");
        checks++;

        selector = $test$plusargs("syn025_override") ? 4'b0000 : 4'b1011;
        casez (selector) matches
            4'b1??1: result = 1;
            default: result = 0;
        endcase
        if (result !== 1) $fatal(1, "casez item question marks");
        checks++;
        casez (selector) matches
            4'b1zz1: result = 1;
            default: result = 0;
        endcase
        if (result !== 1) $fatal(1, "casez item Z");
        checks++;
        casex (selector) matches
            4'b1xx1: result = 1;
            default: result = 0;
        endcase
        if (result !== 1) $fatal(1, "casex item X");
        checks++;
        casez (selector) matches
            4'b1xx1: result = 1;
            default: result = 0;
        endcase
        if (result !== 0) $fatal(1, "casez item X is exact");
        checks++;
        case (selector) matches
            4'b1zz1: result = 1;
            default: result = 0;
        endcase
        if (result !== 0) $fatal(1, "exact item Z is not wildcard");
        checks++;
        case (selector) matches
            4'b1??1: result = 1;
            default: result = 0;
        endcase
        if (result !== 0) $fatal(1, "exact item question marks are Z");
        checks++;
        case (selector) matches
            4'b1xx1: result = 1;
            default: result = 0;
        endcase
        if (result !== 0) $fatal(1, "exact item X is not wildcard");
        checks++;

        packet = '{tag: 4'h3,
                   payload: ($test$plusargs("syn025_payload_known") ? 4'b1001 : 4'b10z1)};
        casez (sampled_packet()) matches
            '{tag: 4'h3, payload: 4'b1001}: result = 1;
            default: result = 0;
        endcase
        if (result !== 1 || calls != 7) $fatal(1, "structure payload casez");
        checks++;
        case (sampled_packet()) matches
            '{tag: 4'h3, payload: 4'b1001}: result = 1;
            default: result = 0;
        endcase
        if (result !== 0 || calls != 8) $fatal(1, "structure payload exact");
        checks++;

        if (checks != 21) $fatal(1, "check count");
        $display("runtime_modes=pass checks=%0d calls=%0d", checks, calls);
        $finish(0);
    end
endmodule
