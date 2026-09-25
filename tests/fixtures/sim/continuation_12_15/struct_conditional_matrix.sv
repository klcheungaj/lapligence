// Immediate structure members merge independently; nested members default whole.
module record_conditional_check #(parameter int W = 7)(output bit done);
    typedef struct {
        logic [W-1:0] data = '1;
        bit flag = 1;
        logic [7:0] stable = 8'h22;
        logic [7:0] row[2] = '{8'h10, 8'h11};
    } record_t;
    typedef struct { record_t child; logic [7:0] stable; } outer_t;
    localparam record_t A = '{data:{W{1'b1}}, flag:1, stable:8'h5a, row:'{8'h10, 8'h11}};
    localparam record_t B = '{data:{W{1'b0}}, flag:0, stable:8'h5a, row:'{8'h10, 8'h12}};
    localparam record_t FOLDED = 1'bx ? A : B;
    localparam record_t KNOWN = 4'b1xz0 ? A : B;
    localparam outer_t OA = '{A, 8'hc3};
    localparam outer_t OB = '{B, 8'hc3};
    localparam outer_t NESTED = 1'bz ? OA : OB;
    record_t a, b, result, queued, live;
    logic [3:0] selector;
    int left_calls, right_calls;

    function automatic record_t choose(input logic [3:0] sel, input record_t l, r);
        return sel ? l : r;
    endfunction
    function automatic record_t left_value();
        left_calls++;
        return a;
    endfunction
    function automatic record_t right_value();
        right_calls++;
        return b;
    endfunction
    function automatic record_t effects(input logic [3:0] sel);
        return sel ? left_value() : right_value();
    endfunction
    function automatic logic [7:0] folded_function();
        record_t r;
        r = 1'bx ? A : B;
        return r.stable;
    endfunction
    localparam logic [7:0] FROM_FUNCTION = folded_function();

    task automatic check_merge(input record_t value);
        if (value.data !== {W{1'bx}} || value.flag !== 0 || value.stable !== 8'h5a
            || value.row[0] !== 8'hxx || value.row[1] !== 8'hxx)
            $fatal(1, "SYN004 W=%0d: member merge or type default", W);
    endtask
    always_comb live = selector ? a : b;
    initial begin
        done = 0;
        a = A;
        b = B;
        selector = 0;
        left_calls = 0;
        right_calls = 0;
        check_merge(FOLDED);
        if (KNOWN !== A || FROM_FUNCTION !== 8'h5a) $fatal(1, "SYN004 constant evaluation");
        if (NESTED.child.flag !== 0 || NESTED.child.stable !== 8'hxx
            || NESTED.child.data !== {W{1'bx}} || NESTED.stable !== 8'hc3)
            $fatal(1, "SYN004 nested immediate default");
        #1;
        if (live !== B) $fatal(1, "SYN004 false arm");
        selector = 4'b1xz0;
        #1;
        if (live !== A) $fatal(1, "SYN004 dominant one");
        result = effects(selector);
        if (result !== A || left_calls != 1 || right_calls != 0)
            $fatal(1, "SYN004 chosen arm evaluation");
        selector = 4'b000x;
        #1;
        check_merge(live);
        result = effects(selector);
        check_merge(result);
        if (left_calls != 2 || right_calls != 1) $fatal(1, "SYN004 ambiguous arm counts");
        result = choose(4'b000z, a, b);
        check_merge(result);
        queued <= choose(selector, a, b);
        a = B;
        b = B;
        #1;
        check_merge(queued);
        if (live !== B) $fatal(1, "SYN004 content-only dependency");
        a.stable = 8'h00;
        #1;
        if (live.stable !== 8'hxx || live.data !== B.data || live.flag !== B.flag)
            $fatal(1, "SYN004 independent member dependency");
        done = 1;
    end
endmodule
module tb;
    wire [4:0] done;
    record_conditional_check #(1) c1(done[0]);
    record_conditional_check #(7) c7(done[1]);
    record_conditional_check #(33) c33(done[2]);
    record_conditional_check #(65) c65(done[3]);
    record_conditional_check #(129) c129(done[4]);
    initial begin
        wait (&done);
        $display("STRUCT_POLICY_PASS");
        $finish(0);
    end
endmodule
