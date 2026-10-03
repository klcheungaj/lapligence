// IEEE 1800-2009 12.6: a whole-value binding of a fixed array beyond packed
// capacity is legal, but patterns have no descriptor transport yet, so the
// source is rejected with its size instead of being flattened.
module tb;
    logic [7:0] table_data [0:200000];
    initial begin
        table_data[5] = 8'h33;
        if (table_data matches .copy) $display("%h", copy[5]);
    end
endmodule
