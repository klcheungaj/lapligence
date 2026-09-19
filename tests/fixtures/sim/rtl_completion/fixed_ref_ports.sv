typedef struct { int count; logic [7:0] data; } value_t;
module child(ref value_t value, output logic [7:0] observed);
    always_comb observed = value.data;
    initial begin
        #2;
        value.count += 5;
        value.data[3:0] = 4'ha;
    end
endmodule
module tb;
    value_t values[2];
    logic [7:0] observed;
    child u(values[1], observed);
    initial begin
        values[0] = '{count:1, data:8'h11};
        values[1] = '{count:2, data:8'h34};
        #1;
        $display("before=%h", observed);
        #2;
        $display("after=%0d,%h observed=%h sibling=%0d,%h", values[1].count, values[1].data,
            observed, values[0].count, values[0].data);
        $finish(0);
    end
endmodule
