// llg-test-fixture: SYN-019 Verilog-2001 rejection of a whole unpacked value
// IEEE 1800-2009 §§7.4.2 and 7.6; a Verilog memory may be indexed, not copied
// as a first-class value.
module tb;
    reg [7:0] a [0:1];
    reg [7:0] b [0:1];
    initial begin
        a[0] = 8'h12;
        a[1] = 8'h34;
        b = a;
        $finish;
    end
endmodule
