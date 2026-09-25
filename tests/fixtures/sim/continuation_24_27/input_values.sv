// llg-test-fixture: source/selector-only updates and one reached input evaluation.
module value_sink(input logic signed [64:0] p [4:5], output wire [129:0] y);
    assign y = {p[4], p[5]};
endmodule
module tb;
    typedef logic signed [64:0] lane_t;
    typedef lane_t row_t [-1:-2];
    row_t a, b;
    row_t matrix [1:0];
    lane_t triple [2:0];
    logic choose;
    integer index;
    integer calls = 0;
    integer before_calls;
    wire [129:0] direct_y, row_y, cond_y, slice_y, pattern_y, call_y;
    function automatic row_t make_row(input row_t x);
        calls++;
        return x;
    endfunction
    value_sink direct(.p(a), .y(direct_y));
    value_sink selected(.p(matrix[index]), .y(row_y));
    value_sink conditional_value(.p(choose ? a : b), .y(cond_y));
    value_sink sliced(.p(triple[2:1]), .y(slice_y));
    value_sink patterned(.p('{a[-1], b[-2]}), .y(pattern_y));
    value_sink called(.p(make_row(a)), .y(call_y));
    task automatic check;
        if (direct_y !== {a[-1],a[-2]} || call_y !== direct_y)
            $fatal(1,"direct/function input");
        if (row_y !== {matrix[index][-1],matrix[index][-2]}) $fatal(1,"selected row");
        if (slice_y !== {triple[2],triple[1]}) $fatal(1,"slice order");
        if (pattern_y !== {a[-1],b[-2]}) $fatal(1,"pattern input");
    endtask
    initial begin
        a = '{-65'sd2, 65'sd4}; b = '{-65'sd2, 65'sd7};
        matrix[1] = '{65'sd10,65'sd11}; matrix[0] = '{65'sd12,65'sd13};
        triple = '{65'sd1,65'sd2,65'sd3}; index = 1; choose = 0;
        #1; check();
        if (cond_y !== {b[-1],b[-2]}) $fatal(1,"false input arm");
        before_calls = calls;
        index = 0;
        #1; check();
        if (calls != before_calls) $fatal(1,"unrelated row selector woke function input");
        matrix[0][-2] = 65'sd21;
        #1; check();
        a[-2] = 65'sd15;
        #1; check();
        if (calls != before_calls+1) $fatal(1,"input expression evaluated per element");
        choose = 1'bx;
        #1;
        if (cond_y !== {-65'sd2,{65{1'bx}}}) $fatal(1,"array conditional member merge");
        choose = 1;
        #1;
        if (cond_y !== {a[-1],a[-2]}) $fatal(1,"true input arm");
        $display("INPUT_VALUES_PASS"); $finish(0);
    end
endmodule
