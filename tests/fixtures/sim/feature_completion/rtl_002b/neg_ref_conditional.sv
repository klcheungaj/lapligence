// IEEE 1800-2009 §13.5.2: a ref actual must be a variable, not a conditional value.
module tb;
    typedef logic [16:0] row_t [0:65536];
    row_t left_value, right_value;
    logic choose;
    function automatic void change(ref row_t value);
        value[0] = 1;
    endfunction
    initial begin
        change(choose ? left_value : right_value);
        $finish(0);
    end
endmodule
