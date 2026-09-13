module tb;
    import "DPI-C" vector_id = function int vector_id(input logic [7:0] value);

    initial begin
        vector_id(8'h5a);
        $finish;
    end
endmodule
