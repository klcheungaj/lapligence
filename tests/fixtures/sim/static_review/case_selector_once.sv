// llg-test-fixture: tests/fixtures/sim/static_review/case_selector_once.sv
// Static-review regression source; not executed in the review session.
module tb;
    integer calls, hit, item_calls;
    function automatic integer selector();
        calls = calls + 1;
        return 5;
    endfunction
    function automatic integer item(input integer value);
        item_calls = item_calls + 1;
        return value;
    endfunction
    initial begin
        calls = 0;
        case (selector())
            0: hit = 0;
            5: hit = 5;
            default: hit = -1;
        endcase
        $display("packed calls=%0d hit=%0d", calls, hit);
        calls = 0;
        item_calls = 0;
        case (selector())
            0.5, item(5), item(6): hit = 5;
            default: hit = -1;
        endcase
        $display("real calls=%0d hit=%0d items=%0d", calls, hit, item_calls);
        $finish(0);
    end
endmodule
