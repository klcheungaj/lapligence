module tb;
    logic [3:0] value;

    function automatic void bump(ref logic [3:0] alias_value);
        alias_value = 4'd9;
    endfunction

    initial begin
        value = 4'd1;
        bump(value);
        $display("value=%0d", value);
        $finish(0);
    end
endmodule
