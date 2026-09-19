typedef struct { logic [7:0] high; logic [7:0] low; } pair_t;
module child(input pair_t source, output wire pair_t result);
    assign result.high = source.low;
    assign result.low = source.high;
endmodule
module tb;
    pair_t source;
    wire pair_t result;
    child u(source, result);
    initial begin
        source = '{high:8'ha5, low:8'h5a};
        #1;
        $display("result=%h,%h", result.high, result.low);
        source.low = 8'h12;
        #1;
        $display("result=%h,%h", result.high, result.low);
        $finish(0);
    end
endmodule
