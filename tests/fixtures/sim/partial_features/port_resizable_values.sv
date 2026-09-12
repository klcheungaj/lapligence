module child(input int in_values[], output int out_values[]);
    always_comb begin
        out_values = in_values;
    end
endmodule

module tb;
    int source[];
    int result[];

    child dut(.in_values(source), .out_values(result));

    initial begin
        source = new[3];
        source[0] = 10;
        source[1] = 20;
        source[2] = 30;
        #1 $display("%0d %0d %0d", result[0], result[1], result[2]);
        source[1] = 77;
        #1 $display("%0d %0d %0d", result[0], result[1], result[2]);
        $finish(0);
    end
endmodule
