module tb;
    logic [7:0] value;

    function automatic void bad(const ref logic [7:0] source);
        source = 8'h01;
    endfunction

    initial begin
        bad(value);
        $finish(0);
    end
endmodule
