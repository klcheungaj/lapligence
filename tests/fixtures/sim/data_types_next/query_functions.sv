// IEEE 1800-2009 20.6-20.7: executed type, packed/unpacked dimension, and
// array query functions retain declared bounds and runtime container shape.
module tb;
    parameter integer P = 7;

    logic [7:0] fixed [7:4];
    logic [0:7] ascending [0:1];
    integer dynamic[];
    integer queue[$];
    integer signed_keys[logic signed [7:0]];
    string text;
    integer dimension;

    initial begin
        dynamic = new[3];
        queue = '{11, 22};
        signed_keys[-3] = 1;
        signed_keys[5] = 2;
        text = "abc";
        dimension = 1;

        if ($left(fixed) !== 7 || $right(fixed) !== 4 ||
            $low(fixed) !== 4 || $high(fixed) !== 7 ||
            $increment(fixed) !== 1 || $size(fixed) !== 4 ||
            $left(fixed, 2) !== 7 || $right(fixed, 2) !== 0 ||
            $dimensions(fixed) !== 2 || $unpacked_dimensions(fixed) !== 1 ||
            $bits(fixed) !== 32) begin
            $display("FAIL query_functions fixed");
            $finish;
        end

        if ($left(ascending) !== 0 || $right(ascending) !== 1 ||
            $increment(ascending) !== -1 || $size(ascending) !== 2) begin
            $display("FAIL query_functions ascending");
            $finish;
        end

        if ($left(dynamic) !== 0 || $right(dynamic) !== 2 ||
            $low(dynamic) !== 0 || $high(dynamic) !== 2 ||
            $increment(dynamic) !== -1 || $size(dynamic) !== 3 ||
            $bits(dynamic) !== 96 || $dimensions(dynamic) !== 2 ||
            $unpacked_dimensions(dynamic) !== 1 ||
            $left(dynamic, dimension) !== 0) begin
            $display("FAIL query_functions dynamic");
            $finish;
        end

        dimension = 3;
        if (!$isunknown($left(dynamic, dimension))) begin
            $display("FAIL query_functions dynamic_dimension");
            $finish;
        end

        if ($left(queue) !== 0 || $right(queue) !== 1 ||
            $size(queue) !== 2 || $bits(queue) !== 64 ||
            $dimensions(queue) !== 2) begin
            $display("FAIL query_functions queue");
            $finish;
        end

        if ($left(signed_keys) !== 0 || $right(signed_keys) !== -1 ||
            $low(signed_keys) !== -3 || $high(signed_keys) !== 5 ||
            $increment(signed_keys) !== -1 || $size(signed_keys) !== 2 ||
            $dimensions(signed_keys) !== 2) begin
            $display("FAIL query_functions associative");
            $finish;
        end

        if ($left(text) !== 0 || $right(text) !== 2 ||
            $low(text) !== 0 || $high(text) !== 2 ||
            $increment(text) !== -1 || $size(text) !== 3 ||
            $bits(text) !== 24 || $dimensions(text) !== 1 ||
            $unpacked_dimensions(text) !== 0) begin
            $display("FAIL query_functions string");
            $finish;
        end

        if ($typename(logic [7:0]) != "logic[7:0]") begin
            $display("FAIL query_functions typename");
            $finish;
        end
        if ($isunbounded(P) !== 0) begin
            $display("FAIL query_functions isunbounded");
            $finish;
        end

        $display("PASS query_functions");
        $finish;
    end
endmodule
