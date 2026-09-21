// SYN-004: direct unpacked-structure conditionals retain immediate member
// boundaries. LRM: IEEE 1800-2009 7.2 and 11.4.11.
module tb;
    typedef struct {
        logic [7:0] byte_value = 8'h11;
        bit flag = 1'b1;
        logic [3:0] equal_value = 4'h2;
        logic [3:0] row [2] = '{4'ha, 4'hb};
    } record_t;
    typedef struct {
        record_t child;
        logic [3:0] lane [2];
    } nested_t;
    typedef struct packed {
        logic [7:0] byte_value;
        logic [3:0] equal_value;
    } packed_t;
    record_t left, right, result;
    record_t array_left [2], array_right [2], array_result [2];
    record_t local_result;
    nested_t nested_left, nested_right, nested_result;
    packed_t packed_left, packed_right, packed_result;
    logic selector, second_selector;
    int left_calls, right_calls;

    function automatic record_t choose(input logic sel, input record_t a, b);
        return sel ? a : b;
    endfunction

    function automatic record_t choose_local(input logic sel);
        record_t value;
        value = sel ? left : right;
        return value;
    endfunction

    function automatic record_t left_arm();
        left_calls++;
        return left;
    endfunction

    function automatic record_t right_arm();
        right_calls++;
        return right;
    endfunction

    function automatic record_t choose_effects(input logic sel);
        return sel ? left_arm() : right_arm();
    endfunction

    function automatic record_t choose_predicate(input logic a, b);
        return a &&& b ? left_arm() : right_arm();
    endfunction

    function automatic nested_t choose_nested(input logic sel, input nested_t a, b);
        return sel ? a : b;
    endfunction

    initial begin
        left.byte_value = 8'ha5;
        right.byte_value = 8'ha6;
        left.flag = 1'b0;
        right.flag = 1'b1;
        left.equal_value = 4'h2;
        right.equal_value = 4'h2;
        left.row[0] = 4'h1;
        left.row[1] = 4'h2;
        right.row[0] = 4'h1;
        right.row[1] = 4'h3;

        selector = 1'b0;
        result = choose(selector, left, right);
        if (result.byte_value !== 8'ha6 || result.flag !== 1'b1 ||
            result.equal_value !== 4'h2 || result.row[1] !== 4'h3)
            $fatal(1, "known false");
        selector = 1'b1;
        result = choose(selector, left, right);
        if (result.byte_value !== 8'ha5 || result.flag !== 1'b0 ||
            result.equal_value !== 4'h2 || result.row[1] !== 4'h2)
            $fatal(1, "known true");

        selector = 1'bx;
        result = choose(selector, left, right);
        if (result.byte_value !== 8'hxx || result.flag !== 1'b0 ||
            result.equal_value !== 4'h2 || result.row[0] !== 4'hx ||
            result.row[1] !== 4'hx)
            $fatal(1, "immediate member merge");

        selector = 1'bz;
        result = choose(selector, left, right);
        if (result.byte_value !== 8'hxx || result.flag !== 1'b0 ||
            result.equal_value !== 4'h2)
            $fatal(1, "highz selector");

        local_result = selector ? left : right;
        if (local_result.byte_value !== 8'hxx || local_result.flag !== 1'b0 ||
            local_result.equal_value !== 4'h2)
            $fatal(1, "local conditional");
        local_result = choose_local(selector);
        if (local_result.byte_value !== 8'hxx || local_result.flag !== 1'b0 ||
            local_result.equal_value !== 4'h2)
            $fatal(1, "automatic local conditional");

        selector = 1'bx;
        left_calls = 0;
        right_calls = 0;
        result = choose_effects(selector);
        if (left_calls != 1 || right_calls != 1 || result.byte_value !== 8'hxx)
            $fatal(1, "ambiguous arm evaluation");
        selector = 1'b1;
        result = choose_effects(selector);
        if (left_calls != 2 || right_calls != 1 || result.byte_value !== 8'ha5)
            $fatal(1, "known arm evaluation");

        second_selector = 1'b0;
        left_calls = 0;
        right_calls = 0;
        result = choose_predicate(1'bx, second_selector);
        if (left_calls != 1 || right_calls != 1 || result.byte_value !== 8'hxx)
            $fatal(1, "predicate merge");

        nested_left.child = left;
        nested_right.child = left;
        nested_left.child.byte_value = 8'ha5;
        nested_right.child.byte_value = 8'ha6;
        nested_left.lane[0] = 4'h1;
        nested_left.lane[1] = 4'h2;
        nested_right.lane[0] = 4'h1;
        nested_right.lane[1] = 4'h3;
        nested_result = choose_nested(1'bx, nested_left, nested_right);
        if (nested_result.child.byte_value !== 8'hxx ||
            nested_result.child.flag !== 1'b0 ||
            nested_result.child.equal_value !== 4'hx ||
            nested_result.lane[0] !== 4'hx || nested_result.lane[1] !== 4'hx)
            $fatal(1, "nested member merge");

        array_left[0] = left;
        array_right[0] = right;
        array_left[1] = left;
        array_right[1] = left;
        selector = 1'bx;
        array_result = selector ? array_left : array_right;
        if (array_result[0].byte_value !== 8'hxx ||
            array_result[0].equal_value !== 4'hx ||
            array_result[1].equal_value !== 4'h2)
            $fatal(1, "R01 array conditional control");

        packed_left = '{byte_value:8'ha5, equal_value:4'h2};
        packed_right = '{byte_value:8'ha6, equal_value:4'h2};
        selector = 1'bx;
        packed_result = selector ? packed_left : packed_right;
        if (packed_result.byte_value !== 8'b101001xx ||
            packed_result.equal_value !== 4'h2)
            $fatal(1, "packed mux control");

        $display("record conditional passed");
        $finish(0);
    end
endmodule
