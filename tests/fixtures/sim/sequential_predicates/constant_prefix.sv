module tb;
    int calls, selected;
    logic [7:0] result;
    function automatic logic zero_with_effect();
        calls++;
        return 0;
    endfunction
    initial begin
        calls = 0; selected = 0;
        result = 1'b0 &&& zero_with_effect() ? 8'ha5 : 8'ha6;
        if (result !== 8'ha6 || calls != 0) $fatal(1, "false prefix");
        result = 1'bx &&& zero_with_effect() ? 8'ha5 : 8'ha6;
        if (result !== 8'b101001xx || calls != 0) $fatal(1, "X prefix");
        result = 1'b1 &&& 1'bz &&& zero_with_effect() ? 8'ha5 : 8'ha6;
        if (result !== 8'b101001xx || calls != 0) $fatal(1, "Z prefix");
        if (1'bx &&& zero_with_effect()) selected = 1; else selected = 2;
        if (calls != 0 || selected != 2) $fatal(1, "ambiguous if");
        result = zero_with_effect() &&& 1'b0 ? 8'ha5 : 8'ha6;
        if (calls != 1 || result !== 8'ha6) $fatal(1, "reached effect erased by later zero");
        // Ordinary && must still inspect its right operand after ambiguous truth.
        result = (1'bx && zero_with_effect()) ? 8'ha5 : 8'ha6;
        if (calls != 2 || result !== 8'ha6) $fatal(1, "ordinary AND changed");
        $display("constant_prefix=pass calls=%0d", calls);
        $finish(0);
    end
endmodule
