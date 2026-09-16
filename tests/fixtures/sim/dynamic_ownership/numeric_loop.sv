module tb;
    logic [64:0] value;
    function automatic logic [64:0] increment(input logic [64:0] input_value);
        return input_value + 65'd1;
    endfunction
    initial begin
        value = 65'd7;
        repeat (1000) value = increment(value);
        $display("%0d", value);
    end
endmodule
