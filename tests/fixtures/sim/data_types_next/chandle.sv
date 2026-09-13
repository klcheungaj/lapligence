// IEEE 1800-2009 6.14: chandles default to null and support only handle/null
// assignment, equality, Boolean tests, and subroutine passing/return.
module tb;
    chandle first;
    chandle second;

    function chandle pass_through(input chandle value);
        pass_through = value;
    endfunction

    function automatic chandle local_copy(input chandle value);
        chandle saved;
        saved = value;
        local_copy = saved;
    endfunction

    function chandle mixed_copy(input logic [7:0] tag, input chandle value);
        chandle saved;
        if (tag == 8'h5a)
            saved = value;
        else
            saved = null;
        mixed_copy = saved;
    endfunction

    function chandle static_copy(input chandle value);
        chandle saved;
        saved = value;
        static_copy = saved;
    endfunction

    task automatic copy_directions(
        input chandle source,
        output chandle output_value,
        inout chandle inout_value,
        ref chandle ref_value,
        const ref chandle observed
    );
        output_value = source;
        inout_value = source;
        ref_value = source;
        if (observed !== source) begin
            $display("FAIL chandle const_ref");
            $finish;
        end
    endtask

    task automatic delayed_copy(input chandle source, output chandle output_value);
        #0;
        output_value = source;
    endtask

    typedef struct {
        chandle handle;
        logic [1:0] tag;
    } chandle_box_t;

    chandle_box_t box_a;
    chandle_box_t box_b;
    chandle output_value;

    initial begin
        if (first != null || first !== null) begin
            $display("FAIL chandle default_null");
            $finish;
        end
        if (first) begin
            $display("FAIL chandle default_boolean");
            $finish;
        end

        second = first;
        if (second != null || second !== first) begin
            $display("FAIL chandle assignment");
            $finish;
        end
        if (second) begin
            $display("FAIL chandle assignment_boolean");
            $finish;
        end
        first = null;
        second = pass_through(first);
        if (!(first == second) || !(first === null)) begin
            $display("FAIL chandle comparison_subroutine");
            $finish;
        end
        if (second) begin
            $display("FAIL chandle subroutine_boolean");
            $finish;
        end

        if (local_copy(first) !== null || mixed_copy(8'h5a, first) !== null
            || mixed_copy(8'h00, first) !== null || static_copy(first) !== null) begin
            $display("FAIL chandle local_or_mixed_return");
            $finish;
        end

        box_a.handle = first;
        box_a.tag = 2'b01;
        box_b = box_a;
        if (box_b.handle !== first || box_b.tag !== 2'b01) begin
            $display("FAIL chandle aggregate_copy");
            $finish;
        end

        output_value = null;
        copy_directions(first, output_value, second, second, first);
        if (output_value !== null || second !== null) begin
            $display("FAIL chandle direction_alias");
            $finish;
        end
        delayed_copy(first, output_value);
        #0;
        if (output_value !== null) begin
            $display("FAIL chandle delayed_copy");
            $finish;
        end

        $display("PASS chandle");
        $finish;
    end
endmodule
