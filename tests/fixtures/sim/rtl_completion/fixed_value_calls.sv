module tb;
    typedef int array_t [2:-1];
    array_t source, result, changed;
    function automatic array_t increment(input array_t a);
        array_t local_values;
        local_values = a;
        foreach (local_values[i]) local_values[i] += 1;
        return local_values;
    endfunction
    task automatic copy_and_change(input array_t a, inout array_t b, output array_t c);
        c = a;
        b[1] = a[-1] + 10;
    endtask
    initial begin
        source[2] = 1; source[1] = 2; source[0] = 3; source[-1] = 4;
        result = increment(source);
        changed = source;
        copy_and_change(result, changed, result);
        $display("source=%0d,%0d,%0d,%0d", source[2], source[1], source[0], source[-1]);
        $display("result=%0d,%0d,%0d,%0d changed=%0d", result[2], result[1], result[0], result[-1], changed[1]);
        $finish(0);
    end
endmodule
