// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/legacy_cp_replacements.sv
// IEEE 1800-2009 §§6.19, 6.21, 7.2, 9.2.2.2, 10.3 and 13.4.1.
// Each function result below is tied to the focal source slot named in the
// SYN-038 replacement vector. The receiving sample variables are independent.
module tb;
    typedef logic [7:0] byte_t;
    typedef struct packed {
        byte_t high;
        byte_t low;
    } pair_t;
    typedef byte_t byte_array_t [0:2];

    pair_t pair_source;
    pair_t pair_result;
    byte_t conditional_left_result;
    byte_t conditional_right_result;

    function automatic pair_t copy_pair(input pair_t value);
        return value;
    endfunction

    byte_array_t array_result;

    function automatic byte_array_t make_array_result();
        make_array_result[0] = 8'h12;
        make_array_result[1] = 8'h34;
        make_array_result[2] = 8'h56;
        return make_array_result;
    endfunction

    function automatic byte_t select_function_value(
        input logic select_right,
        input byte_t left_value,
        input byte_t right_value
    );
        return select_right ? right_value : left_value;
    endfunction

    logic [7:0] net_seed;
    wire [7:0] continuous_net_source;
    logic [7:0] net_function_sample;
    logic [7:0] net_direct_sample;

    assign continuous_net_source = net_seed;

    function automatic logic [7:0] read_continuous_net();
        return continuous_net_source;
    endfunction

    assign net_function_sample = read_continuous_net();
    assign net_direct_sample = continuous_net_source;

    logic [7:0] variable_seed;
    logic [7:0] continuous_variable_source;
    logic [7:0] variable_function_sample;
    logic [7:0] variable_direct_sample;

    assign continuous_variable_source = variable_seed;

    function automatic logic [7:0] read_continuous_variable();
        return continuous_variable_source;
    endfunction

    assign variable_function_sample = read_continuous_variable();
    assign variable_direct_sample = continuous_variable_source;

    logic [7:0] comb_input;
    logic [7:0] comb_state;
    logic [7:0] comb_function_sample;

    function automatic logic [7:0] read_comb_state();
        return comb_state;
    endfunction

    always_comb begin
        comb_state = comb_input;
        comb_function_sample = read_comb_state();
    end

    logic clk;
    logic [7:0] ff_next_state;
    logic [7:0] ff_state = 8'h11;
    logic [7:0] ff_function_sample;

    function automatic logic [7:0] read_ff_state();
        return ff_state;
    endfunction

    always_ff @(posedge clk) begin
        ff_state <= ff_next_state;
        ff_function_sample <= read_ff_state();
    end

    initial begin
        pair_source = '{high:8'h12, low:8'h34};
        pair_result = copy_pair(pair_source);
        array_result = make_array_result();
        if (pair_result !== pair_source ||
            array_result[0] !== 8'h12 ||
            array_result[1] !== 8'h34 ||
            array_result[2] !== 8'h56)
            $fatal(1, "typed function return or return-array element write mismatch");

        conditional_left_result = select_function_value(1'b0, 8'h21, 8'h43);
        conditional_right_result = select_function_value(1'b1, 8'h21, 8'h43);
        if (conditional_left_result !== 8'h21 ||
            conditional_right_result !== 8'h43)
            $fatal(1, "conditional function result mismatch");

        net_seed = 8'h5a;
        variable_seed = 8'h7d;
        comb_input = 8'h6b;
        #1;
        if (continuous_net_source !== 8'h5a ||
            net_function_sample !== 8'h5a ||
            net_direct_sample !== 8'h5a ||
            continuous_variable_source !== 8'h7d ||
            variable_function_sample !== 8'h7d ||
            variable_direct_sample !== 8'h7d ||
            comb_state !== 8'h6b ||
            comb_function_sample !== 8'h6b)
            $fatal(1, "continuous or combinational function source mismatch");
        $display("initial=12/34 array=12,34,56 conditional=21,43 net=5a function=5a direct=5a var=7d function=7d direct=7d comb=6b/6b");

        net_seed = 8'ha5;
        variable_seed = 8'hc6;
        comb_input = 8'h7c;
        #1;
        if (continuous_net_source !== 8'ha5 ||
            net_function_sample !== 8'ha5 ||
            net_direct_sample !== 8'ha5 ||
            continuous_variable_source !== 8'hc6 ||
            variable_function_sample !== 8'hc6 ||
            variable_direct_sample !== 8'hc6 ||
            comb_state !== 8'h7c ||
            comb_function_sample !== 8'h7c)
            $fatal(1, "updated continuous or combinational function source mismatch");
        $display("updated=net=a5 function=a5 direct=a5 var=c6 function=c6 direct=c6 comb=7c/7c");

        ff_next_state = 8'h22;
        clk = 1'b0;
        #1 clk = 1'b1;
        #1;
        if (ff_state !== 8'h22 || ff_function_sample !== 8'h11)
            $fatal(1, "function did not read the pre-NBA state value");
        $display("ff=11->22 sample=11");

        ff_next_state = 8'h33;
        clk = 1'b0;
        #1 clk = 1'b1;
        #1;
        if (ff_state !== 8'h33 || ff_function_sample !== 8'h22)
            $fatal(1, "function did not read the second pre-NBA state value");
        $display("ff=22->33 sample=22");
        $finish(0);
    end
endmodule
