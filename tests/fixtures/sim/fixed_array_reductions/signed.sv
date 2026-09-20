module tb;
    typedef enum logic signed [7:0] { NEG3 = -3, NEG4 = -4 } enum_t;
    typedef struct packed signed { logic [7:0] bits; } packed_t;
    logic signed [7:0] data [1:0];
    enum_t states [0:1];
    packed_t records [0:1];
    longint result;
    initial begin
        data[1] = -3; data[0] = -4;
        result = data.sum();
        states[0] = NEG3; states[1] = NEG4;
        records[0].bits = 8'hfd; records[1].bits = 8'hfc;
        $display("sum=%0d product=%0d wide=%0d enum=%0d record=%0d", data.sum(),
                 data.product(), result, states.sum(), records.sum());
        $display("mapped=%0d width=%0d", data.sum(v) with (longint'(v)),
                 $bits(data.sum(v) with (longint'(v))));
        $finish(0);
    end
endmodule
