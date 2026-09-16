module tb;
    int calls;
    logic [64:0] value, other;
    function automatic logic [64:0] hit();
        calls = calls + 1;
        return 65'd99;
    endfunction
    initial begin
        calls = 0;
        value = 1'b0 && hit();
        value = 1'b1 || hit();
        value = 1'b1 ? 65'd23 : hit();
        $display("%0d %0d", value, calls);
        value = {65{1'bz}};
        other = {65{1'bz}};
        value = 1'bx ? value : other;
        $display("%0d", value === {65{1'bz}});
    end
endmodule
