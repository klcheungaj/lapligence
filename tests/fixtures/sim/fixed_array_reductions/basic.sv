// IEEE 1800-2009 7.12.3: all five methods on runtime fixed storage.
module tb;
    logic [7:0] data [-2:0];
    initial begin
        data[-2] = 2; data[-1] = 3; data[0] = 4;
        $display("sum=%0d product=%0d and=%0d or=%0d xor=%0d bits=%0d",
                 data.sum, data.product(), data.and, data.or(), data.xor, $bits(data.sum()));
        data[-1] = 7;
        $display("changed=%0d", data.sum());
    end
endmodule
