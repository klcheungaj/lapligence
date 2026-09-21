// IEEE 1800-2009 7.12.4: zero is outside the defined iterator dimensions.
module tb;
    logic [7:0] data [1:3];
    int result;
    initial begin
        data[1] = 1; data[2] = 2; data[3] = 3;
        result = data.sum(item) with (item.index(0));
        $display("result=%0d", result);
        $finish(0);
    end
endmodule
