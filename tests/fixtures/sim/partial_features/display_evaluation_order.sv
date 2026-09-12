// llg-test-fixture: tests/fixtures/sim/partial_features/display_evaluation_order.sv
module tb;
    int calls;

    function automatic int next_value();
        calls = calls + 1;
        return calls;
    endfunction

    initial begin
        calls = 0;
        $display("args=%0d,%0d calls=%0d", next_value(), next_value(), calls);
        $finish(0);
    end
endmodule
