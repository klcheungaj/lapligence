module child(input logic [7:0] data [2:1], output int total);
    always_comb total = data.sum(value) with (int'(value) + value.index());
endmodule
module tb;
    logic [7:0] source [-1:0];
    wire signed [31:0] result;
    child dut(.data(source), .total(result));
    initial begin
        source[-1] = 200; source[0] = 56;
        #1 $display("total=%0d", result);
        source[-1] = 1;
        #1 $display("total=%0d", result);
        source[0] = 2;
        #1 $display("total=%0d", result);
    end
endmodule
