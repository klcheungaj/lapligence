// llg-test-fixture: tests/fixtures/sim/data_types_next/syn_015_stream_width_matrix.sv
// IEEE 1800-2009 §§6.24.1, 6.24.3, 11.4.12, 11.4.14.1-11.4.14.4: fixed
// bit-stream casts and streams across widths whose slices do not divide the
// stream. Every oracle below uses only concatenation and selects.
module stream_case #(parameter int W = 7)(output bit done);
    typedef logic [W-1:0] lane_t;
    typedef bit [W-1:0] bit_lane_t;
    typedef lane_t pair_t [0:1];
    typedef lane_t reversed_pair_t [1:0];
    typedef bit_lane_t bit_pair_t [0:1];
    typedef lane_t trio_t [0:2];
    typedef logic [2*W-1:0] flat_t;
    typedef struct { logic [W:0] high; logic [W-2:0] low; } split_t;
    typedef struct { lane_t tag; pair_t lanes; } record_t;
    typedef union tagged packed { void none; logic signed [3:0] value; } signed_tag_t;

    lane_t a, b, c;
    pair_t pair;
    reversed_pair_t reversed;
    bit_pair_t two_state;
    split_t split;
    record_t record_value;
    trio_t trio;
    pair_t matrix [1:0];
    lane_t window [0:3];
    flat_t flat, expected_flat;
    logic [2*W+3:0] padded;
    logic [W+3:0] initialized = {>>{lane_t'('h123456789abcdef0123456789abcdef01)}};
    logic signed [3:0] narrow_signed;
    int widened;
    signed_tag_t tagged_value;
    logic [7:0] tagged_stream;
    integer calls;

    function automatic pair_t make_pair(input lane_t left, input lane_t right);
        calls = calls + 1;
        return '{left, right};
    endfunction

    function automatic int singleton_cast(input logic signed [3:0] value);
        return int'({value});
    endfunction

    // `<< 8` slices from the right; the leftmost block keeps the remainder.
    function automatic flat_t byte_reverse(input flat_t value);
        int position;
        int size;
        position = 2 * W;
        for (int start = 0; start < 2 * W; start += 8) begin
            size = (2 * W - start < 8) ? 2 * W - start : 8;
            for (int bit_index = size - 1; bit_index >= 0; bit_index--) begin
                position--;
                byte_reverse[position] = value[start + bit_index];
            end
        end
    endfunction

    task automatic check(input bit condition, input string label);
        if (!condition) $fatal(1, "SYN015 W=%0d %s", W, label);
    endtask

    initial begin
        done = 0;
        calls = 0;
        a = lane_t'('h5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5);
        b = lane_t'('h0f1e2d3c4b5a69788796a5b4c3d2e1f0f);
        c = lane_t'('h123456789abcdef0123456789abcdef01);
        pair = '{a, b};

        // Left-bound element first in both directions.
        flat = flat_t'(pair);
        check(flat === {a, b}, "array to packed cast");
        reversed = reversed_pair_t'(pair);
        check(reversed[1] === a && reversed[0] === b, "array to reversed array cast");
        split = split_t'(pair);
        check({split.high, split.low} === {a, b}, "non-dividing member cast");
        record_value = '{c, pair};
        trio = trio_t'(record_value);
        check(trio[0] === c && trio[1] === a && trio[2] === b, "record to array cast");
        record_value = record_t'(trio_t'('{b, c, a}));
        check(record_value.tag === b && record_value.lanes[0] === c &&
              record_value.lanes[1] === a, "array to record cast");

        // Four-state to two-state conversion happens per bit.
        pair[0][0] = 1'bx;
        pair[1][W-1] = 1'bz;
        two_state = bit_pair_t'(pair);
        check(two_state[0] === {a[W-1:1], 1'b0} && two_state[1] === {1'b0, b[W-2:0]},
              "four-state to two-state array cast");
        pair = '{a, b};

        // Selected rows, members and call results are sources.
        matrix[1] = pair;
        matrix[0] = '{b, a};
        check(flat_t'(matrix[0]) === {b, a}, "selected row cast");
        check(flat_t'(record_value.lanes) === {c, a}, "selected member cast");
        check(flat_t'(make_pair(c, b)) === {c, b} && calls == 1, "call result cast");

        // Streams: `>>` keeps order; `<<` reverses slices from the right.
        flat = {>>{pair}};
        check(flat === {a, b}, "left-to-right stream");
        flat = {<<{pair}};
        expected_flat = {a, b};
        for (int index = 0; index < 2 * W; index++)
            check(flat[index] === expected_flat[2 * W - 1 - index], "bit-reverse stream");
        flat = {<<8{pair}};
        check(flat === byte_reverse({a, b}), "partial leftmost slice");
        flat = {<<lane_t{pair}};
        check(flat === {b, a}, "type slice");

        // A wider target is left-aligned and zero-filled on the right.
        padded = {>>{pair}};
        check(padded === {a, b, 4'b0000}, "left-aligned stream padding");
        trio = {>>{b, c}};
        check(trio[0] === b && trio[1] === c && trio[2] === '0,
              "left-aligned unpacked stream target");
        check(initialized === {c, 4'b0000}, "left-aligned declaration initializer");

        // Unpack consumes the leftmost bits; unused low bits are ignored.
        {>>{window[0], window[3]}} = {c, a, 4'b1111};
        check(window[0] === c && window[3] === a, "unpack from the left");

        // `with` selects a one-dimensional range for pack and unpack.
        window = '{a, b, c, a};
        check(flat_t'({>>{window with [1:2]}}) === {b, c}, "with pack");
        {>>{window with [2 +: 2]}} = {b, b};
        check(window[0] === a && window[1] === b && window[2] === b && window[3] === b,
              "with unpack leaves other elements");

        // A stream target snapshots its source before any write.
        {>>{pair[1], pair[0]}} = {>>{pair}};
        check(pair[1] === a && pair[0] === b, "overlapping stream snapshot");

        // A singleton concatenation is unsigned before a following cast.
        narrow_signed = -4'sd2;
        widened = singleton_cast(narrow_signed);
        check(widened == 14, "singleton concatenation cast");
        check(int'(narrow_signed) == -2, "signed cast control");

        // A valid signed tagged member keeps its sign through a cast, while a
        // singleton concatenation and a stream treat it as unsigned bits.
        tagged_value = tagged value (-4'sd2);
        check(int'(tagged_value.value) == -2, "signed tagged member cast");
        check(int'({tagged_value.value}) == 14, "tagged member singleton concatenation");
        tagged_stream = {>>{tagged_value.value}};
        check(tagged_stream === 8'he0, "tagged member stream");
        done = 1;
    end
endmodule

module tb;
    wire [2:0] done;
    stream_case #(7) narrow(done[0]);
    stream_case #(65) wide(done[1]);
    stream_case #(129) wider(done[2]);
    initial begin
        #1;
        if (done !== 3'b111) $fatal(1, "incomplete stream matrix");
        $display("SYN015_STREAM_WIDTH_MATRIX_PASS");
        $finish(0);
    end
endmodule
