// SV 11.4.13: traverse unpacked arrays, not packed casts; RHS-only wildcards.
module inside_check #(parameter W = 7) (output bit done);
    typedef logic signed [W-1:0] lane_t;
    typedef lane_t row_t [1:0];
    typedef lane_t matrix_t [1:0][-1:0];
    typedef logic [2*W-1:0] packed_t;
    typedef bit [W-1:0] bitrow_t [1:0];
    typedef struct { row_t values; logic [7:0] other; } holder_t;
    row_t stored, other;
    matrix_t matrix;
    holder_t holder;
    logic signed [W+3:0] key;
    logic choose;
    logic found;
    integer calls;

    function automatic row_t make_row(input lane_t a, input lane_t b);
        calls++;
        return '{a, b};
    endfunction

    function automatic matrix_t make_matrix(input lane_t a, input lane_t b);
        return '{'{a, b}, '{b, a}};
    endfunction

    task automatic check(input logic value, input logic expected);
        if (value !== expected) $fatal(1, "inside result W=%0d got=%b expected=%b", W, value, expected);
    endtask

    initial begin
        done = 0;
        calls = 0;
        stored = '{lane_t'(-1), lane_t'(0)};
        other = '{lane_t'(1), lane_t'(0)};
        key = -1;
        check(key inside {stored}, 1);
        check(key inside {make_row(lane_t'(-1), lane_t'(0))}, 1);
        if (calls != 1) $fatal(1, "one reached array-producing call");
        choose = 1;
        check(key inside {choose ? stored : other}, 1);
        check(key inside {make_matrix(lane_t'(-1), lane_t'(0))}, 1);
        matrix = '{stored, other};
        check(key inside {matrix[1]}, 1);
        holder.values = stored;
        holder.other = 8'h55;
        check(key inside {holder.values}, 1);
        // A packed bit-stream cast contributes a single value, not source cells.
        check(packed_t'(stored) inside {packed_t'(stored)}, 1);
        check(packed_t'(stored) inside {stored}, 0);
        // Conversion to two-state array values must not be bypassed via storage.
        stored = '{lane_t'('x), lane_t'('x)};
        key = 1;
        check(key inside {stored}, 1);
        check(key inside {bitrow_t'(stored)}, 0);
        // Unknown LHS is not a wildcard, but a definite RHS match dominates X.
        key = 'x;
        stored = '{lane_t'(0), lane_t'(1)};
        check(key inside {make_row(stored[1], stored[0])}, 1'bx);
        stored[0] = 'z;
        check(key inside {make_row(stored[1], stored[0])}, 1'b1);
        done = 1;
    end
endmodule
module tb;
    real existing_real_storage[0:1];
    wire [2:0] done;
    inside_check #(7) c7(done[0]);
    inside_check #(65) c65(done[1]);
    inside_check #(129) c129(done[2]);
    initial begin
        existing_real_storage[0]=2.5;
        existing_real_storage[1]=3.5;
        if (!(2.5 inside {existing_real_storage})) $fatal(1, "retained real array storage path");
        wait (&done);
        $display("INSIDE_VALUE_CONTEXTS_PASS");
        $finish(0);
    end
endmodule
