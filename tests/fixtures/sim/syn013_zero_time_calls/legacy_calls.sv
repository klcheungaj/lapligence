// llg-test-fixture: tests/fixtures/sim/syn013_zero_time_calls/legacy_calls.sv
// IEEE 1364-2001 §§10.2.2-10.2.3 and 10.3.1-10.3.3: delay-free task and
// automatic function activations keep their formal values and return storage.
module tb;
    reg [7:0] value;
    reg [7:0] result;
    integer calls;

    function automatic [7:0] increment;
        input [7:0] source;
        begin
            calls = calls + 1;
            increment = source + 1'b1;
        end
    endfunction

    task automatic copy_twice;
        input [7:0] source;
        output [7:0] destination;
        begin
            destination = increment(increment(source));
        end
    endtask

    initial begin
        calls = 0;
        value = 8'd40;
        copy_twice(value, result);
        $display("legacy value=%0d result=%0d calls=%0d", value, result, calls);
        $finish(0);
    end
endmodule
