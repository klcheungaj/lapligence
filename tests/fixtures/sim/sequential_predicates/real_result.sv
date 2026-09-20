module tb;
    logic a, b;
    real result;
    int left_calls, right_calls;
    function automatic real left_value();
        left_calls++;
        return 2.5;
    endfunction
    function automatic real right_value();
        right_calls++;
        return 3.5;
    endfunction
    initial begin
        left_calls = 0; right_calls = 0;
        a = 1'bx; b = 0;
        result = a &&& b ? left_value() : right_value();
        if (result != 0.0 || left_calls != 1 || right_calls != 1)
            $fatal(1, "ambiguous real must evaluate both arms and yield zero");
        a = 1; b = 1;
        result = a &&& b ? left_value() : right_value();
        if (result != 2.5 || left_calls != 2 || right_calls != 1) $fatal(1, "known true real");
        a = 0; b = 1'bz;
        result = a &&& b ? left_value() : right_value();
        if (result != 3.5 || left_calls != 2 || right_calls != 2) $fatal(1, "known false real");
        a = 1'bz;
        result = a ? left_value() : right_value();
        if (result != 0.0 || left_calls != 3 || right_calls != 3) $fatal(1, "ordinary real mux");
        $display("real_result=pass calls=%0d,%0d", left_calls, right_calls);
        $finish(0);
    end
endmodule
