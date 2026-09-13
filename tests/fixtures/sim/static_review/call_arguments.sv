// llg-test-fixture: tests/fixtures/sim/static_review/call_arguments.sv
// Static-review regression source; not executed in the review session.
module tb;
    integer left_value, right_value, result;
    function automatic integer pair(output integer left, output integer right);
        left = 10;
        right = 20;
        return left + right;
    endfunction
    task automatic increment(ref integer value);
        value = value + 1;
    endtask
    initial begin
        result = pair(left_value, right_value);
        increment(left_value);
        $display("left=%0d right=%0d result=%0d", left_value, right_value, result);
        $finish(0);
    end
endmodule
