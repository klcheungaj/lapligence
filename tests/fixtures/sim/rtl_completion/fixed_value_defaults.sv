module tb;
    typedef struct { int count; logic [7:0] data; } value_t;
    value_t values [0:2];
    function automatic value_t make();
        make.data = 8'h12;
    endfunction
    task automatic fill(output value_t result);
        result.data = 8'h34;
    endtask
    task fill_static(output value_t result);
        result.data = 8'h56;
    endtask
    initial begin
        values[0] = make();
        fill(values[1]);
        fill_static(values[2]);
        if (values[0] !== value_t'{count:0, data:8'h12}) $fatal(1, "return defaults");
        if (values[1] !== value_t'{count:0, data:8'h34}) $fatal(1, "automatic output defaults");
        if (values[2] !== value_t'{count:0, data:8'h56}) $fatal(1, "static output defaults");
        $display("fixed defaults passed");
        $finish(0);
    end
endmodule
