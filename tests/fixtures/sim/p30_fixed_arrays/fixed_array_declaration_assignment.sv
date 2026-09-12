// P30 fixed unpacked-array declaration initializers.
module tb;
    logic [7:0] source [0:3] = '{8'h11, 8'h22, 8'h33, 8'h44};
    logic [7:0] target [0:3] = source;
    logic [7:0] slice_copy [0:1] = source[2:3];

    initial begin
        if (target[0] !== 8'h11 || target[1] !== 8'h22 ||
            target[2] !== 8'h33 || target[3] !== 8'h44) begin
            $display("FAIL fixed_array_declaration whole_copy");
            $finish;
        end
        if (slice_copy[0] !== 8'h33 || slice_copy[1] !== 8'h44) begin
            $display("FAIL fixed_array_declaration slice_copy");
            $finish;
        end
        $display("PASS fixed_array_declaration");
        $finish;
    end
endmodule
