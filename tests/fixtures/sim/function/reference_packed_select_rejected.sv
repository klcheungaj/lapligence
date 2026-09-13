module tb;
    logic [7:0] packed_value;
    function automatic void set_bit(ref logic value);
        value = 1'b1;
    endfunction
    initial begin
        packed_value = 0;
        set_bit(packed_value[2]);
        $finish(0);
    end
endmodule
