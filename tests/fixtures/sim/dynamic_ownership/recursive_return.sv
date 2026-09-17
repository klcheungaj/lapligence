module tb;
    logic [64:0] total;
    function automatic logic [64:0] descend(input int depth);
        logic [64:0] local_value;
        if (depth == 0) return 65'd1;
        local_value = descend(depth - 1);
        return local_value + 65'd1;
    endfunction
    initial begin
        total = 0;
        repeat (128) total = total + descend(16);
        $display("%0d", total);
        $finish(0);
    end
endmodule
