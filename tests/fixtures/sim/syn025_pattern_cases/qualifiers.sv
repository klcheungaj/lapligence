// llg-test-fixture: tests/fixtures/sim/syn025_pattern_cases/qualifiers.sv
// IEEE 1800-2009 12.6.1 and 12.5.3: item filters affect qualifier counts.
module tb;
    logic [7:0] value, result;
    int calls;

    function automatic logic [7:0] sampled();
        calls = calls + 1;
        sampled = value;
    endfunction

    initial begin
        value = 8'h5a;
        result = 0;
        calls = 0;
        unique case (sampled()) matches
            .a &&& a == 8'h00: result = 1;
            .b &&& b == 8'h5a: result = b;
            .c &&& c == 8'h5a: result = 2;
        endcase
        if (result !== 8'h5a || calls != 1) $fatal(1, "filtered unique order/capture");

        unique case (value) matches
            .a: result = 3;
            .b: result = 4;
            default: result = 5;
        endcase
        if (result !== 3) $fatal(1, "unique first body");

        unique0 case (value) matches
            .a: result = 6;
            .b: result = 7;
        endcase
        if (result !== 6) $fatal(1, "unique0 first body");

        value = 8'h00;
        unique case (value) matches
            8'h01: result = 8;
        endcase
        if (result !== 6) $fatal(1, "unique no match retains value");

        unique0 case (value) matches
            8'h01: result = 8;
        endcase
        if (result !== 6) $fatal(1, "unique0 no match retains value");

        priority case (value) matches
            8'h01: result = 8;
        endcase
        if (result !== 6) $fatal(1, "priority no match retains value");

        priority case (value) matches
            .a: result = 0;
            .b: result = 9;
        endcase
        if (result !== 0) $fatal(1, "priority first body");

        unique case (value) matches
            8'h01: result = 1;
            default: result = 0;
        endcase
        priority case (value) matches
            8'h01: result = 1;
            default: result = 0;
        endcase
        if (result !== 0) $fatal(1, "qualified default");

        $display("qualifiers=pass result=%0d calls=%0d", result, calls);
        $finish(0);
    end
endmodule
