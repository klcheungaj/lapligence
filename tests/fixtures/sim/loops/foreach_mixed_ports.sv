// llg-test-fixture: tests/fixtures/sim/loops/foreach_mixed_ports.sv
// IEEE 1800-2009 12.7.3, 23: preserve formal bounds and array sensitivity.
module child(input logic [3:0] a [2:1], output integer sum);
    always_comb begin
        sum = 0;
        foreach (a[i,j]) sum += (i * 10 + j) * a[i][j];
    end
endmodule

module tb;
    logic [3:0] source [0:1];
    wire signed [31:0] sum;
    child dut(.a(source), .sum(sum));
    initial begin
        source[0] = 4'b1010;
        source[1] = 4'b0101;
        #1 $display("sum=%0d", sum);
        source[0][1] = 0;
        #1 $display("sum=%0d", sum);
        source[1] = 4'b1111;
        #1 $display("sum=%0d", sum);
        $finish(0);
    end
endmodule
