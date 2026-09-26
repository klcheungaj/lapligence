// IEEE 1800-2009 7.12.3-7.12.4: the array method and iterator query are
// SystemVerilog syntax; the surrounding declarations are legal Verilog-2001.
module tb;
    reg [7:0] values [2:4];
    integer result;
    initial begin
        values[2] = 0; values[3] = 0; values[4] = 0;
        result = values.sum(item) with (item.index());
        $display("indices=%0d", result);
        $finish(0);
    end
endmodule
