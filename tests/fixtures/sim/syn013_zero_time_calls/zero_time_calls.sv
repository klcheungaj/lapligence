// llg-test-fixture: tests/fixtures/sim/syn013_zero_time_calls/zero_time_calls.sv
// IEEE 1800-2009 §§6.21-6.22 and 13.3-13.5: fixed zero-time calls retain
// declaration identity, per-activation storage, named/default argument
// capture, and reference identity across nested calls.

typedef logic [7:0] lane_t;
typedef lane_t row_t [-1:1];

function automatic row_t make_row(input int base, input int step = base + 1);
    row_t value;
    value[-1] = base;
    value[0] = base + step;
    value[1] = base + step + 1;
    return value;
endfunction

function automatic int read_row(const ref row_t value);
    read_row = value[-1] + value[0] + value[1];
endfunction

function automatic int forward_row(const ref row_t value);
    forward_row = read_row(value);
endfunction

task automatic mutate_row(ref row_t value, output row_t snapshot);
    snapshot = value;
    value[-1] = value[-1] + 1;
endtask

task automatic leave_local_block(output logic [7:0] value);
    begin : local_exit
        value = 8'd7;
        disable local_exit;
        value = 8'd99;
    end
endtask

module worker #(parameter integer BIAS = 0)(output integer observed);
    function integer next_value(input integer amount);
        static integer state = 0;
        automatic integer current;
        state = state + amount + BIAS;
        current = state;
        next_value = current;
    endfunction

    initial begin
        observed = next_value(1);
        observed = observed + next_value(1);
    end
endmodule

module tb;
    row_t source;
    row_t result;
    row_t snapshot;
    logic [7:0] selected;
    integer left;
    integer right;
    integer forwarded;
    worker #(1) first(left);
    worker #(10) second(right);

    initial begin
        source = make_row(.base(4));
        result = make_row(.step(2), .base(4));
        mutate_row(result, snapshot);
        forwarded = forward_row(snapshot);
        selected = result[-1];
        leave_local_block(selected);
        #1;
        $display("default=%0d,%0d,%0d row=%0d,%0d,%0d snapshot=%0d,%0d,%0d forwarded=%0d selected=%0d workers=%0d,%0d",
                 source[-1], source[0], source[1],
                 result[-1], result[0], result[1],
                 snapshot[-1], snapshot[0], snapshot[1], forwarded, selected,
                 left, right);
        $finish(0);
    end
endmodule
