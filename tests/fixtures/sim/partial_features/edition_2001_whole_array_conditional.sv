// llg-test-fixture: SYN-019 Verilog-2001 rejection of array-valued conditional
// IEEE 1800-2009 §11.4.11; a memory is not a Verilog-2001 expression value.
module tb;
    reg [7:0] a [0:1];
    reg [7:0] b [0:1];
    reg select;
    initial begin
        select = 1'b1;
        b = select ? a : b;
        $finish;
    end
endmodule
