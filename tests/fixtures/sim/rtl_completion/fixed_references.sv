module tb;
    typedef int array_t [2:-1];
    array_t value;
    task automatic step(ref array_t a, const ref array_t observer, input int depth);
        a[1] += 1;
        if (observer[1] != value[1]) $fatal(1, "reference is not immediate");
        if (depth > 0) step(a, observer, depth-1);
    endtask
    initial begin
        value[2]=1; value[1]=2; value[0]=3; value[-1]=4;
        step(value, value, 2);
        $display("value=%0d,%0d,%0d,%0d", value[2], value[1], value[0], value[-1]);
        $finish(0);
    end
endmodule
