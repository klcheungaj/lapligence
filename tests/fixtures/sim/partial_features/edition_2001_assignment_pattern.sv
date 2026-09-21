// llg-test-fixture: SYN-019 Verilog-2001 rejection of an assignment pattern
// IEEE 1800-2009 §10.9; the pattern expression is not in Verilog-2001.
module tb;
    reg [3:0] value;
    initial begin
        value = '{default: 1'b0};
        $finish;
    end
endmodule
