typedef struct { int count; logic [7:0] data[2]; } value_t;
module child(input value_t source, output value_t result);
    always_comb begin
        result = source;
        result.count = source.count + 1;
        result.data[1] = source.data[0] ^ source.data[1];
    end
endmodule
module tb;
    value_t source[2], result[2];
    child u(source[1], result[0]);
    initial begin
        source[1] = '{count:7, data:'{8'h5a,8'ha5}};
        #1;
        $display("count=%0d data=%h,%h", result[0].count, result[0].data[0], result[0].data[1]);
        source[1].data[0] = 8'h12;
        #1;
        $display("count=%0d data=%h,%h", result[0].count, result[0].data[0], result[0].data[1]);
        $finish(0);
    end
endmodule
