// IEEE 1800-2009 7.6: assigning a resizable array to a fixed unpacked array
// requires a runtime-compatible element count and leaves the destination
// unchanged when the shape check fails.
module tb;
    logic [7:0] target [0:3];
    logic [7:0] dynamic_values[];

    initial begin
        target = '{8'haa, 8'hbb, 8'hcc, 8'hdd};
        dynamic_values = new[2];
        dynamic_values = '{8'h11, 8'h22};
        target = dynamic_values;
        $display("FAIL fixed_array_shape_mismatch accepted");
        $finish;
    end
endmodule
