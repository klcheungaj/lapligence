// llg-test-fixture: tests/fixtures/sim/rtl_completion/syn_006_array_continuous.sv
// LRM: IEEE 1800-2009 §§6.6-6.7, 7.6, 10.3; IEEE 1800-2005 §6.5.
module tb;
    timeunit 1ns;
    timeprecision 1ps;

    typedef logic [7:0] array_t [0:1];

    logic select;
    logic [7:0] a [0:1];
    logic [7:0] b [0:1];
    wire [7:0] net_y [0:1];
    logic [7:0] variable_y [0:1];
    wire [7:0] conditional_y [0:1];
    wire [7:0] pattern_y [0:1];
    wire [7:0] selected_y [0:1];
    logic [7:0] split_y [0:1];

    bit [7:0] bit_a [0:1];
    bit [7:0] bit_y [0:1];

    logic [7:0] matrix_a [0:1][0:1];
    wire [7:0] matrix_y [0:1][0:1];

    array_t function_base;
    wire [7:0] function_y [0:1];
    integer function_calls;

    assign net_y = a;
    assign net_y = b;
    assign variable_y = a;
    assign conditional_y = select ? a : b;
    assign pattern_y = '{8'hc1, 8'hc2};
    assign selected_y[1] = a[1];
    assign split_y[0] = a[0];
    assign split_y[1] = b[1];
    assign bit_y = bit_a;
    assign matrix_y[1] = matrix_a[1];
    assign function_y = make_array(function_base, 8'h01);

    function automatic array_t make_array(input array_t value, input logic [7:0] add);
        array_t result;
        begin
            function_calls = function_calls + 1;
            result = value;
            result[1] = result[1] + add;
            return result;
        end
    endfunction

    initial begin
        function_calls = 0;
        select = 1'bx;
        a[0] = 8'ha1;
        a[1] = 8'ha2;
        b[0] = 8'ha1;
        b[1] = 8'hb2;
        bit_a[0] = 8'h01;
        bit_a[1] = 8'h02;
        matrix_a[0][0] = 8'h11;
        matrix_a[0][1] = 8'h12;
        matrix_a[1][0] = 8'h31;
        matrix_a[1][1] = 8'h32;
        function_base[0] = 8'he1;
        function_base[1] = 8'he2;
        #1;
        $display("t1 net=%h,%h var=%h,%h cond=%h,%h", net_y[0], net_y[1], variable_y[0], variable_y[1], conditional_y[0], conditional_y[1]);
        $display("t1 pattern=%h,%h selected=%h,%h split=%h,%h bit=%h,%h row=%h,%h func=%h,%h calls=%0d", pattern_y[0], pattern_y[1], selected_y[0], selected_y[1], split_y[0], split_y[1], bit_y[0], bit_y[1], matrix_y[1][0], matrix_y[1][1], function_y[0], function_y[1], function_calls);

        select = 1'b1;
        b[0] = 8'hc1;
        b[1] = 8'hc2;
        a[1] = 8'hd2;
        bit_a[0] = 8'h03;
        matrix_a[1][0] = 8'h41;
        function_base[1] = 8'he3;
        #1;
        $display("t2 net=%h,%h var=%h,%h cond=%h,%h", net_y[0], net_y[1], variable_y[0], variable_y[1], conditional_y[0], conditional_y[1]);
        $display("t2 pattern=%h,%h selected=%h,%h split=%h,%h bit=%h,%h row=%h,%h func=%h,%h calls=%0d", pattern_y[0], pattern_y[1], selected_y[0], selected_y[1], split_y[0], split_y[1], bit_y[0], bit_y[1], matrix_y[1][0], matrix_y[1][1], function_y[0], function_y[1], function_calls);
        $finish(0);
    end
endmodule
